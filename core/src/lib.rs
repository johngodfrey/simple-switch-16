//! Switch 16 (simplified) — pure game rules.
//!
//! This crate knows nothing about HTTP, storage or where random numbers come
//! from. Randomness is injected through the [`Dice`] trait so the rules are
//! deterministic under test and portable to any host (native, WASI, browser).
//!
//! ## Rules
//! * Each player works through cards `1..=16`, starting at card 1.
//! * On their turn a player rolls N dice: 3 dice for cards 1–6, 4 for 7–11,
//!   5 for 12–16 (N is fixed by the current card *at the time of the roll*).
//! * The player claims the current card by choosing unused dice whose values
//!   sum to the card's number. Those dice are spent and the next card becomes
//!   current — the player may keep claiming from the remaining dice.
//! * When no dice can make the current card the turn ends
//!   automatically. A player may also pass voluntarily.
//! * A player wins by claiming card 16. The result is the number of rolls taken.

use serde::{Deserialize, Serialize};
use std::fmt;

pub const FIRST_CARD: u8 = 1;
pub const FINAL_CARD: u8 = 16;
pub const DIE_FACES: u8 = 6;

/// Number of dice rolled when the player's current card is `card`.
pub fn dice_for_card(card: u8) -> usize {
    match card {
        0..=6 => 3,
        7..=11 => 4,
        _ => 5,
    }
}

/// A source of die rolls. Implementations must return values in `1..=6`.
pub trait Dice {
    fn roll(&mut self) -> u8;
}

impl<F: FnMut() -> u8> Dice for F {
    fn roll(&mut self) -> u8 {
        self()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub name: String,
    /// The card this player is currently trying to make (1..=16).
    pub card: u8,
    /// Number of rolls this player has made.
    pub rolls: u32,
}

impl Player {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            card: FIRST_CARD,
            rolls: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Die {
    pub value: u8,
    pub used: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnPhase {
    /// The current player must roll.
    AwaitingRoll,
    /// The current player has dice on the board and may claim or pass.
    Claiming,
}

/// What happened to the current roll's dice when the turn ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnEnd {
    /// No combination of unused dice could make the current card.
    NoCombination,
    /// The player chose to pass.
    Passed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    /// The dice from the most recent roll. Kept after a turn ends so the UI
    /// can show what was rolled.
    pub dice: Vec<Die>,
    pub phase: TurnPhase,
    /// Set when the previous turn ended, cleared on the next roll.
    pub last_turn_end: Option<TurnEnd>,
}

impl Board {
    fn new() -> Self {
        Self {
            dice: Vec::new(),
            phase: TurnPhase::AwaitingRoll,
            last_turn_end: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum GameStatus {
    InProgress,
    Won { winner: usize, rolls: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Game {
    pub players: Vec<Player>,
    pub current_player: usize,
    pub board: Board,
    pub status: GameStatus,
}

/// A player command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Move {
    /// Roll the dice for the current card.
    Roll,
    /// Spend the dice at these indices (into `board.dice`) to make the current card.
    Claim { dice: Vec<usize> },
    /// Give up the rest of this roll.
    Pass,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum GameError {
    GameOver,
    MustRollFirst,
    AlreadyRolled,
    NoDiceSelected,
    DieOutOfRange { index: usize },
    DieSelectedTwice { index: usize },
    WrongTotal { expected: u8, actual: u32 },
    InvalidDieValue { value: u8 },
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameError::GameOver => write!(f, "the game is already over"),
            GameError::MustRollFirst => write!(f, "you need to roll before claiming or passing"),
            GameError::AlreadyRolled => write!(f, "you have already rolled; claim a card or pass"),
            GameError::NoDiceSelected => write!(f, "select at least one die"),
            GameError::DieOutOfRange { index } => write!(f, "there is no die at position {index}"),
            GameError::DieSelectedTwice { index } => write!(f, "die {index} was selected twice"),
            GameError::WrongTotal { expected, actual } => {
                write!(
                    f,
                    "those dice add up to {actual}, but the card is {expected}"
                )
            }
            GameError::InvalidDieValue { value } => {
                write!(f, "dice produced invalid value {value}")
            }
        }
    }
}

impl std::error::Error for GameError {}

impl Game {
    pub fn new(player_names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut players: Vec<Player> = player_names.into_iter().map(Player::new).collect();
        if players.is_empty() {
            players.push(Player::new("Player 1"));
        }
        Self {
            players,
            current_player: 0,
            board: Board::new(),
            status: GameStatus::InProgress,
        }
    }

    pub fn current(&self) -> &Player {
        &self.players[self.current_player]
    }

    /// Whether some subset of the unused dice sums to the current card.
    pub fn can_claim(&self) -> bool {
        let dice: Vec<u8> = self.board.dice.iter().map(|d| d.value).collect();
        self.board.phase == TurnPhase::Claiming
            && self.status == GameStatus::InProgress
            && subset_sums_to(&dice, self.current().card)
    }

    /// Number of dice the current player will roll next.
    pub fn next_roll_dice(&self) -> usize {
        dice_for_card(self.current().card)
    }

    fn end_turn(&mut self, reason: TurnEnd) {
        self.board.phase = TurnPhase::AwaitingRoll;
        self.board.last_turn_end = Some(reason);
        self.current_player = (self.current_player + 1) % self.players.len();
    }

    /// After a roll or claim, end the turn if nothing more can be claimed.
    fn end_turn_if_stuck(&mut self) {
        if self.status == GameStatus::InProgress && !self.can_claim() {
            self.end_turn(TurnEnd::NoCombination);
        }
    }
}

/// Apply a move. On error the game is left unchanged.
pub fn make_move(game: &mut Game, command: Move, dice: &mut impl Dice) -> Result<(), GameError> {
    if game.status != GameStatus::InProgress {
        return Err(GameError::GameOver);
    }

    match command {
        Move::Roll => {
            if game.board.phase != TurnPhase::AwaitingRoll {
                return Err(GameError::AlreadyRolled);
            }
            let n = game.next_roll_dice();
            let mut rolled = Vec::with_capacity(n);
            for _ in 0..n {
                let value = dice.roll();
                if !(1..=DIE_FACES).contains(&value) {
                    return Err(GameError::InvalidDieValue { value });
                }
                rolled.push(Die { value, used: false });
            }
            game.board.dice = rolled;
            game.board.phase = TurnPhase::Claiming;
            game.board.last_turn_end = None;
            game.players[game.current_player].rolls += 1;
            game.end_turn_if_stuck();
        }

        Move::Claim { dice: selected } => {
            if game.board.phase != TurnPhase::Claiming {
                return Err(GameError::MustRollFirst);
            }
            if selected.is_empty() {
                return Err(GameError::NoDiceSelected);
            }
            let mut seen = vec![false; game.board.dice.len()];
            let mut total: u32 = 0;
            for &index in &selected {
                let die = game
                    .board
                    .dice
                    .get(index)
                    .ok_or(GameError::DieOutOfRange { index })?;
                if seen[index] {
                    return Err(GameError::DieSelectedTwice { index });
                }
                seen[index] = true;
                total += u32::from(die.value);
            }
            let card = game.current().card;
            if total != u32::from(card) {
                return Err(GameError::WrongTotal {
                    expected: card,
                    actual: total,
                });
            }

            // Valid claim: advance the card.
            let idx = game.current_player;
            if card == FINAL_CARD {
                game.status = GameStatus::Won {
                    winner: idx,
                    rolls: game.players[idx].rolls,
                };
                game.board.phase = TurnPhase::AwaitingRoll;
            } else {
                game.players[idx].card += 1;
                game.end_turn_if_stuck();
            }
        }

        Move::Pass => {
            if game.board.phase != TurnPhase::Claiming {
                return Err(GameError::MustRollFirst);
            }
            game.end_turn(TurnEnd::Passed);
        }
    }
    Ok(())
}

/// Subset-sum over at most a handful of small values (≤5 dice, target ≤16).
fn subset_sums_to(values: &[u8], target: u8) -> bool {
    let target = usize::from(target);
    let mut reachable = vec![false; target + 1];
    reachable[0] = true;
    for &v in values {
        let v = usize::from(v);
        for s in (v..=target).rev() {
            if reachable[s - v] {
                reachable[s] = true;
            }
        }
    }
    target > 0 && reachable[target]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dice that replay a fixed script.
    fn scripted(values: &[u8]) -> impl FnMut() -> u8 {
        let mut it = values.to_vec().into_iter();
        move || it.next().expect("script exhausted")
    }

    fn claim(idx: &[usize]) -> Move {
        Move::Claim { dice: idx.to_vec() }
    }

    fn no_dice() -> impl FnMut() -> u8 {
        || panic!("should not roll")
    }

    #[test]
    fn dice_count_steps_up_at_7_and_12() {
        assert_eq!((1..=6).map(dice_for_card).collect::<Vec<_>>(), vec![3; 6]);
        assert_eq!((7..=11).map(dice_for_card).collect::<Vec<_>>(), vec![4; 5]);
        assert_eq!((12..=16).map(dice_for_card).collect::<Vec<_>>(), vec![5; 5]);
    }

    #[test]
    fn new_game_starts_on_card_one_awaiting_roll() {
        let g = Game::new(["John"]);
        assert_eq!(g.current().card, 1);
        assert_eq!(g.current().rolls, 0);
        assert_eq!(g.board.phase, TurnPhase::AwaitingRoll);
        assert_eq!(g.status, GameStatus::InProgress);
    }

    #[test]
    fn chain_claims_from_one_roll() {
        let mut g = Game::new(["John"]);
        make_move(&mut g, Move::Roll, &mut scripted(&[1, 2, 6])).unwrap();
        make_move(&mut g, claim(&[0]), &mut no_dice()).unwrap(); // 1
        make_move(&mut g, claim(&[1]), &mut no_dice()).unwrap(); // 2
                                                                 // Claiming should not consume the roll value(s) used to claim a card,
                                                                 // they should be available for use on the next card
        make_move(&mut g, claim(&[0, 1]), &mut no_dice()).unwrap(); // 3 (from 1 + 2)
                                                                    // Card 4 needs 3 from {1, 2, 6}: impossible → turn ends automatically.
        assert_eq!(g.current().card, 4);
        assert_eq!(g.board.phase, TurnPhase::AwaitingRoll);
        assert_eq!(g.board.last_turn_end, Some(TurnEnd::NoCombination));
    }

    #[test]
    fn roll_with_no_combination_ends_turn_immediately() {
        let mut g = Game::new(["John"]);
        make_move(&mut g, Move::Roll, &mut scripted(&[2, 3, 4])).unwrap();
        assert_eq!(g.board.phase, TurnPhase::AwaitingRoll);
        assert_eq!(g.current().rolls, 1);
        assert_eq!(g.current().card, 1);
    }

    #[test]
    fn multi_die_claim() {
        let mut g = Game::new(["John"]);
        g.players[0].card = 9;
        make_move(&mut g, Move::Roll, &mut scripted(&[4, 5, 1, 1])).unwrap();
        assert_eq!(g.board.dice.len(), 4);
        make_move(&mut g, claim(&[0, 1]), &mut no_dice()).unwrap();
        assert_eq!(g.current().card, 10);
    }

    #[test]
    fn rejects_bad_claims_without_changing_state() {
        let mut g = Game::new(["John"]);
        make_move(&mut g, Move::Roll, &mut scripted(&[1, 1, 2])).unwrap();
        let before = g.clone();
        let d = &mut no_dice();
        assert_eq!(
            make_move(&mut g, claim(&[]), d),
            Err(GameError::NoDiceSelected)
        );
        assert_eq!(
            make_move(&mut g, claim(&[7]), d),
            Err(GameError::DieOutOfRange { index: 7 })
        );
        assert_eq!(
            make_move(&mut g, claim(&[0, 0]), d),
            Err(GameError::DieSelectedTwice { index: 0 })
        );
        assert_eq!(
            make_move(&mut g, claim(&[2]), d),
            Err(GameError::WrongTotal {
                expected: 1,
                actual: 2
            })
        );
        assert_eq!(
            make_move(&mut g, Move::Roll, d),
            Err(GameError::AlreadyRolled)
        );
        assert_eq!(g, before);
    }

    #[test]
    fn must_roll_before_claim_or_pass() {
        let mut g = Game::new(["John"]);
        assert_eq!(
            make_move(&mut g, claim(&[0]), &mut no_dice()),
            Err(GameError::MustRollFirst)
        );
        assert_eq!(
            make_move(&mut g, Move::Pass, &mut no_dice()),
            Err(GameError::MustRollFirst)
        );
    }

    #[test]
    fn pass_ends_turn() {
        let mut g = Game::new(["John"]);
        make_move(&mut g, Move::Roll, &mut scripted(&[1, 1, 1])).unwrap();
        make_move(&mut g, Move::Pass, &mut no_dice()).unwrap();
        assert_eq!(g.board.phase, TurnPhase::AwaitingRoll);
        assert_eq!(g.board.last_turn_end, Some(TurnEnd::Passed));
    }

    #[test]
    fn dice_count_is_fixed_at_roll_time() {
        // On card 6 with 3 dice; claiming 6 moves to card 7 but the same roll continues.
        let mut g = Game::new(["John"]);
        g.players[0].card = 6;
        make_move(&mut g, Move::Roll, &mut scripted(&[6, 3, 4])).unwrap();
        make_move(&mut g, claim(&[0]), &mut no_dice()).unwrap();
        assert_eq!(g.current().card, 7);
        assert_eq!(g.board.dice.len(), 3);
        make_move(&mut g, claim(&[1, 2]), &mut no_dice()).unwrap();
        assert_eq!(g.current().card, 8);
        assert_eq!(g.next_roll_dice(), 4);
    }

    #[test]
    fn claiming_16_wins_and_reports_rolls() {
        let mut g = Game::new(["John"]);
        g.players[0].card = 16;
        g.players[0].rolls = 41;
        make_move(&mut g, Move::Roll, &mut scripted(&[6, 6, 4, 1, 1])).unwrap();
        make_move(&mut g, claim(&[0, 1, 2]), &mut no_dice()).unwrap();
        assert_eq!(
            g.status,
            GameStatus::Won {
                winner: 0,
                rolls: 42
            }
        );
        assert_eq!(
            make_move(&mut g, Move::Roll, &mut no_dice()),
            Err(GameError::GameOver)
        );
    }

    #[test]
    fn invalid_dice_source_is_rejected() {
        let mut g = Game::new(["John"]);
        assert_eq!(
            make_move(&mut g, Move::Roll, &mut scripted(&[7, 1, 1])),
            Err(GameError::InvalidDieValue { value: 7 })
        );
        assert_eq!(g.current().rolls, 0);
    }

    #[test]
    fn full_game_with_a_greedy_bot_terminates() {
        // Simple LCG so the test is deterministic without a rand dependency.
        let mut seed: u32 = 12345;
        let mut dice = move || {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            ((seed >> 16) % 6) as u8 + 1
        };
        let mut g = Game::new(["Bot"]);
        for _ in 0..10_000 {
            if g.status != GameStatus::InProgress {
                break;
            }
            match g.board.phase {
                TurnPhase::AwaitingRoll => make_move(&mut g, Move::Roll, &mut dice).unwrap(),
                TurnPhase::Claiming => {
                    let pick =
                        find_subset(&g.board.dice, g.current().card).expect("can_claim lied");
                    make_move(&mut g, claim(&pick), &mut dice).unwrap();
                }
            }
        }
        match g.status {
            GameStatus::Won { rolls, .. } => assert!(rolls >= 6, "rolls = {rolls}"),
            _ => panic!("game did not finish"),
        }
    }

    fn find_subset(dice: &[Die], target: u8) -> Option<Vec<usize>> {
        let n = dice.len();
        (1u32..(1 << n)).find_map(|mask| {
            let idx: Vec<usize> = (0..n).filter(|i| mask & (1 << i) != 0).collect();
            let ok = idx.iter().all(|&i| !dice[i].used)
                && idx.iter().map(|&i| u32::from(dice[i].value)).sum::<u32>() == u32::from(target);
            ok.then_some(idx)
        })
    }

    #[test]
    fn json_shapes() {
        let m: Move = serde_json::from_str(r#"{"type":"claim","dice":[0,2]}"#).unwrap();
        assert_eq!(m, claim(&[0, 2]));
        let m: Move = serde_json::from_str(r#"{"type":"roll"}"#).unwrap();
        assert_eq!(m, Move::Roll);
        let e = serde_json::to_string(&GameError::WrongTotal {
            expected: 3,
            actual: 4,
        })
        .unwrap();
        assert_eq!(e, r#"{"code":"wrong_total","expected":3,"actual":4}"#);
    }
}
