# Scale16

A simplified **Switch 16** dice game built to practise scale-to-zero architecture:
a Rust game engine compiled to WebAssembly and run by [Spin](https://spinframework.dev),
with a React/TypeScript front end talking to it over HTTP.

```
Browser (React/TS)  ──HTTP──▶  Spin
  board, dice, animations        /api/*  → api component (Rust → wasm32-wasip1)
                                 /*      → static fileserver (frontend/dist)
                                              │
                                              ▼
                                         Spin KV ("default")
```

Each request spins up a fresh Wasm instance, loads the game from KV, applies one
move, saves it and exits. There's no in-memory state, so the app can scale to zero
between requests.

## The game

- Your card starts at **1** and goes up to **16**.
- Roll the dice: **3 dice** for cards 1–6, **4** for 7–11, **5** for 12–16. The
  number is fixed when you roll.
- Select unused dice that add up to your card to **claim** it. The dice you used
  are spent and the next card is now current. Keep claiming from the dice you have left.
- When no combination of the remaining dice can make your card, the turn ends
  automatically. You can also **pass**.
- Claim card 16 to win. The game tells you how many rolls it took.

## Layout

| Path | What it is |
|---|---|
| `core/` | `switch16-core`: the rules only. `Game`, `Player`, `Board`, `Move`, `make_move()`. No I/O; dice come in through a `Dice` trait, so tests are deterministic. |
| `api/src/service.rs` | Request handling against a `GameStore` trait: parse, load, `make_move`, save, reply. Plain Rust, unit tested natively. |
| `api/src/spin.rs` | Spin glue: router, `KvGameStore` (Spin KV) and `SystemDice` (WASI CSPRNG). Only compiled for `wasm32`. |
| `frontend/` | Vite + React 19 + TypeScript UI. |
| `spin.toml` | Two components: `api` on `/api/...` and `web` (fileserver) on `/...`. |

## Prerequisites

```bash
rustup target add wasm32-wasip1
brew install spinframework/tap/spin      # or see https://spinframework.dev/install
node --version                           # 20+
```

## Run it

```bash
spin build --up          # builds the Rust component and the frontend, then serves on :3000
open http://127.0.0.1:3000
```

Game state is kept in `.spin/` (SQLite-backed local KV) and survives restarts.
The game id is in the URL hash, so refreshing resumes the game.

### Front-end dev loop

```bash
spin up --build          # terminal 1: API on :3000 (spin watch also works)
npm --prefix frontend run dev   # terminal 2: Vite on :5173, proxies /api to :3000
```

### Tests

```bash
cargo test               # core rules + service layer (native; no Spin needed)
```

## HTTP API

| Method | Path | Body | Result |
|---|---|---|---|
| `POST` | `/api/games` | `{"player_name"?: "John"}` | `201` game |
| `GET` | `/api/games/:id` | | `200` game / `404` |
| `POST` | `/api/games/:id/moves` | `{"type":"roll"}` · `{"type":"claim","dice":[0,2]}` · `{"type":"pass"}` | `200` game / `422` rule violation / `400` bad JSON |

The game response is the stored `Game` plus derived fields:

```json
{
  "id": "3f9c…",
  "players": [{ "name": "John", "card": 4, "rolls": 3 }],
  "current_player": 0,
  "board": {
    "dice": [{ "value": 3, "used": true }, { "value": 4, "used": false }, { "value": 6, "used": false }],
    "phase": "claiming",
    "last_turn_end": null
  },
  "status": { "state": "in_progress" },
  "can_claim": true,
  "next_roll_dice": 3
}
```

When a game is won, `status` is `{"state":"won","winner":0,"rolls":24}`.

Rule violations return a readable message and a typed code:

```json
{ "error": "those dice add up to 7, but the card is 5",
  "detail": { "code": "wrong_total", "expected": 5, "actual": 7 } }
```

```bash
ID=$(curl -s -XPOST localhost:3000/api/games -d '{"player_name":"John"}' | jq -r .id)
curl -s -XPOST localhost:3000/api/games/$ID/moves -d '{"type":"roll"}' | jq .board
```

## Design notes

- **The rules don't depend on Spin.** `make_move` takes `&mut impl Dice`. The API
  passes the WASI CSPRNG (with rejection sampling for fair d6s), and tests pass
  scripted dice or a seeded LCG. You could move the same crate into the browser via
  `wasm-bindgen` for offline play.
- **Failed moves leave the game untouched.** Every move is validated before
  anything changes, and nothing is saved on error.
- **Multiplayer is ready but not used yet.** `players: Vec<Player>` and
  `current_player` already rotate on turn end. Hot-seat play only needs a UI change.
- **Concurrency.** Spin KV has no compare-and-swap, so two tabs posting moves to the
  same game at the same time would race (last write wins). If that matters, add a
  `version` field and reject stale writes, or move to SQLite with a transaction.
- **SDK version.** This uses `spin-sdk` 5.x (sync API, `wasm32-wasip1`), which runs on
  any Spin 3.x or later. `spin-sdk` 7.x moves to async WASIp3 handlers
  (`#[http_service]`, `wasm32-wasip2`) and needs a recent Spin. That's a contained
  change to `api/src/spin.rs` and `spin.toml` if you want to try it.

## Deploying

Any Spin host works, for example `spin cloud deploy` (Fermyon Cloud), SpinKube on
Kubernetes, or `spin aka deploy`. They all provide a `default` KV store, so you
don't need extra config.
