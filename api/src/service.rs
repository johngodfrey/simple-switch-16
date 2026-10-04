//! Transport- and storage-agnostic request handling.
//!
//! Everything here is plain Rust so it can be unit tested natively with an
//! in-memory store. `lib.rs` wires it to Spin's HTTP trigger and KV store.

use serde::{Deserialize, Serialize};
use switch16_core::{make_move, Dice, Game, GameError, Move};

/// Persistence for games. Spin KV in production, a HashMap in tests.
pub trait GameStore {
    fn load(&self, id: &str) -> anyhow::Result<Option<Game>>;
    fn save(&self, id: &str, game: &Game) -> anyhow::Result<()>;
}

/// What the API returns for a game: the stored state plus derived fields the
/// UI would otherwise have to recompute.
#[derive(Debug, Serialize)]
pub struct GameView<'a> {
    pub id: &'a str,
    #[serde(flatten)]
    pub game: &'a Game,
    pub can_claim: bool,
    pub next_roll_dice: usize,
}

impl<'a> GameView<'a> {
    pub fn new(id: &'a str, game: &'a Game) -> Self {
        Self { id, game, can_claim: game.can_claim(), next_roll_dice: game.next_roll_dice() }
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct CreateGame {
    #[serde(default)]
    pub player_name: Option<String>,
}

/// A response independent of any HTTP library.
#[derive(Debug, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub body: serde_json::Value,
}

impl Reply {
    fn ok(status: u16, id: &str, game: &Game) -> Self {
        Self { status, body: serde_json::to_value(GameView::new(id, game)).expect("serialisable") }
    }

    fn error(status: u16, message: impl Into<String>) -> Self {
        Self { status, body: serde_json::json!({ "error": message.into() }) }
    }

    fn game_error(err: &GameError) -> Self {
        Self {
            status: 422,
            body: serde_json::json!({ "error": err.to_string(), "detail": err }),
        }
    }
}

const MAX_NAME_LEN: usize = 32;

pub fn create_game(
    store: &impl GameStore,
    body: &[u8],
    new_id: impl FnOnce() -> anyhow::Result<String>,
) -> anyhow::Result<Reply> {
    let req: CreateGame = if body.iter().all(u8::is_ascii_whitespace) {
        CreateGame::default()
    } else {
        match serde_json::from_slice(body) {
            Ok(r) => r,
            Err(e) => return Ok(Reply::error(400, format!("invalid JSON: {e}"))),
        }
    };
    let name = req
        .player_name
        .map(|n| n.trim().chars().take(MAX_NAME_LEN).collect::<String>())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "Player 1".to_string());

    let id = new_id()?;
    let game = Game::new([name]);
    store.save(&id, &game)?;
    Ok(Reply::ok(201, &id, &game))
}

pub fn get_game(store: &impl GameStore, id: &str) -> anyhow::Result<Reply> {
    if !is_valid_id(id) {
        return Ok(Reply::error(404, "game not found"));
    }
    Ok(match store.load(id)? {
        Some(game) => Reply::ok(200, id, &game),
        None => Reply::error(404, "game not found"),
    })
}

pub fn post_move(
    store: &impl GameStore,
    id: &str,
    body: &[u8],
    dice: &mut impl Dice,
) -> anyhow::Result<Reply> {
    if !is_valid_id(id) {
        return Ok(Reply::error(404, "game not found"));
    }
    let command: Move = match serde_json::from_slice(body) {
        Ok(m) => m,
        Err(e) => return Ok(Reply::error(400, format!("invalid move: {e}"))),
    };
    let Some(mut game) = store.load(id)? else {
        return Ok(Reply::error(404, "game not found"));
    };
    if let Err(err) = make_move(&mut game, command, dice) {
        return Ok(Reply::game_error(&err));
    }
    store.save(id, &game)?;
    Ok(Reply::ok(200, id, &game))
}

/// Game ids are 32 lowercase hex chars; anything else can't be a key we wrote.
pub fn is_valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

pub fn store_key(id: &str) -> String {
    format!("game:{id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[derive(Default)]
    struct MemStore(RefCell<HashMap<String, String>>);

    impl GameStore for MemStore {
        fn load(&self, id: &str) -> anyhow::Result<Option<Game>> {
            Ok(match self.0.borrow().get(&store_key(id)) {
                Some(s) => Some(serde_json::from_str(s)?),
                None => None,
            })
        }
        fn save(&self, id: &str, game: &Game) -> anyhow::Result<()> {
            self.0.borrow_mut().insert(store_key(id), serde_json::to_string(game)?);
            Ok(())
        }
    }

    const ID: &str = "0123456789abcdef0123456789abcdef";

    fn new_game(store: &MemStore) -> Reply {
        create_game(store, br#"{"player_name":"  John  "}"#, || Ok(ID.to_string())).unwrap()
    }

    #[test]
    fn create_then_get() {
        let store = MemStore::default();
        let r = new_game(&store);
        assert_eq!(r.status, 201);
        assert_eq!(r.body["id"], ID);
        assert_eq!(r.body["players"][0]["name"], "John");
        assert_eq!(r.body["next_roll_dice"], 3);
        assert_eq!(r.body["status"]["state"], "in_progress");

        let r = get_game(&store, ID).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.body["board"]["phase"], "awaiting_roll");
    }

    #[test]
    fn empty_body_creates_default_player() {
        let store = MemStore::default();
        let r = create_game(&store, b"", || Ok(ID.to_string())).unwrap();
        assert_eq!(r.body["players"][0]["name"], "Player 1");
    }

    #[test]
    fn moves_are_persisted() {
        let store = MemStore::default();
        new_game(&store);
        let mut dice = {
            let mut v = vec![6u8, 2, 1].into_iter();
            move || v.next().unwrap()
        };
        let r = post_move(&store, ID, br#"{"type":"roll"}"#, &mut dice).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.body["can_claim"], true);
        let r = post_move(&store, ID, br#"{"type":"claim","dice":[2]}"#, &mut dice).unwrap();
        assert_eq!(r.body["players"][0]["card"], 2);
        let r = get_game(&store, ID).unwrap();
        assert_eq!(r.body["players"][0]["card"], 2);
        assert_eq!(r.body["players"][0]["rolls"], 1);
    }

    #[test]
    fn rule_violation_is_422_and_not_saved() {
        let store = MemStore::default();
        new_game(&store);
        let r = post_move(&store, ID, br#"{"type":"pass"}"#, &mut || 1).unwrap();
        assert_eq!(r.status, 422);
        assert_eq!(r.body["detail"]["code"], "must_roll_first");
    }

    #[test]
    fn bad_input() {
        let store = MemStore::default();
        new_game(&store);
        assert_eq!(post_move(&store, ID, b"{nope", &mut || 1).unwrap().status, 400);
        assert_eq!(post_move(&store, ID, br#"{"type":"dance"}"#, &mut || 1).unwrap().status, 400);
        assert_eq!(get_game(&store, "../etc").unwrap().status, 404);
        assert_eq!(get_game(&store, &"f".repeat(32)).unwrap().status, 404);
    }
}
