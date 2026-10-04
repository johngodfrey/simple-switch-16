//! Spin glue: HTTP routing, KV storage and host randomness.
//! Only compiled for wasm32 so native `cargo test` can exercise `service`.

use crate::service::{self, GameStore, Reply};
use spin_sdk::http::{IntoResponse, Params, Request, Response, Router};
use spin_sdk::http_component;
use spin_sdk::key_value::Store;
use switch16_core::Game;

#[http_component]
fn handle(req: Request) -> Response {
    let mut router = Router::new();
    router.post("/api/games", create_game);
    router.get("/api/games/:id", get_game);
    router.post("/api/games/:id/moves", post_move);
    router.any("/api/*", |_req: Request, _p: Params| {
        to_response(Reply { status: 404, body: serde_json::json!({ "error": "not found" }) })
    });
    router.handle(req)
}

fn create_game(req: Request, _params: Params) -> anyhow::Result<impl IntoResponse> {
    let store = KvGameStore::open()?;
    let reply = service::create_game(&store, req.body(), new_game_id)?;
    Ok(to_response(reply))
}

fn get_game(_req: Request, params: Params) -> anyhow::Result<impl IntoResponse> {
    let store = KvGameStore::open()?;
    let reply = service::get_game(&store, params.get("id").unwrap_or_default())?;
    Ok(to_response(reply))
}

fn post_move(req: Request, params: Params) -> anyhow::Result<impl IntoResponse> {
    let store = KvGameStore::open()?;
    let id = params.get("id").unwrap_or_default();
    let reply = service::post_move(&store, id, req.body(), &mut SystemDice)?;
    Ok(to_response(reply))
}

fn to_response(reply: Reply) -> Response {
    Response::builder()
        .status(reply.status)
        .header("content-type", "application/json")
        .header("cache-control", "no-store")
        .body(reply.body.to_string())
        .build()
}

/// Games stored as JSON in Spin's default key-value store.
struct KvGameStore(Store);

impl KvGameStore {
    fn open() -> anyhow::Result<Self> {
        Ok(Self(Store::open_default()?))
    }
}

impl GameStore for KvGameStore {
    fn load(&self, id: &str) -> anyhow::Result<Option<Game>> {
        Ok(self.0.get_json(service::store_key(id))?)
    }

    fn save(&self, id: &str, game: &Game) -> anyhow::Result<()> {
        Ok(self.0.set_json(service::store_key(id), game)?)
    }
}

/// Fair d6 from the host's CSPRNG (WASI `random_get`), using rejection
/// sampling to avoid modulo bias.
struct SystemDice;

impl switch16_core::Dice for SystemDice {
    fn roll(&mut self) -> u8 {
        loop {
            let mut b = [0u8; 1];
            getrandom::fill(&mut b).expect("host RNG unavailable");
            if b[0] < 252 {
                return b[0] % 6 + 1;
            }
        }
    }
}

fn new_game_id() -> anyhow::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("rng: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
