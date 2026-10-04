//! Spin HTTP component for Switch 16.
//!
//! ```text
//! HTTP request → parse command → load Game (KV) → make_move() → save Game (KV) → JSON
//! ```
//!
//! Routes (mounted under `/api` by spin.toml):
//! * `POST /api/games`            – create a game `{ "player_name"?: string }`
//! * `GET  /api/games/:id`        – fetch a game
//! * `POST /api/games/:id/moves`  – `{ "type": "roll" | "pass" }` or
//!   `{ "type": "claim", "dice": [indices] }`
//!
//! The component is stateless: every request opens the KV store, does its
//! work and exits, so Spin can instantiate it per request and scale to zero.

pub mod service;

#[cfg(target_arch = "wasm32")]
mod spin;
