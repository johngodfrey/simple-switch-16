import { useCallback, useEffect, useState } from "react";
import { api, ApiError, type Game, type Move } from "./api";
import { CardTrack } from "./components/CardTrack";
import { DieFace } from "./components/DieFace";

const ROLL_ANIMATION_MS = 550;

function gameIdFromHash(): string | null {
  const m = window.location.hash.match(/^#\/games\/([0-9a-f]{32})$/);
  return m ? m[1] : null;
}

export default function App() {
  const [game, setGame] = useState<Game | null>(null);
  const [loading, setLoading] = useState(() => gameIdFromHash() !== null);
  const [busy, setBusy] = useState(false);
  const [rolling, setRolling] = useState(false);
  const [selected, setSelected] = useState<number[]>([]);
  const [message, setMessage] = useState<{
    text: string;
    tone: "info" | "good" | "bad";
  } | null>(null);
  const [name, setName] = useState("");

  // Resume a game from the URL (so refresh / share works — state lives in Spin KV).
  useEffect(() => {
    const load = () => {
      const id = gameIdFromHash();
      if (!id) {
        setGame(null);
        setLoading(false);
        return;
      }
      setLoading(true);
      api
        .getGame(id)
        .then(setGame)
        .catch(() => {
          setMessage({ text: "That game could not be found.", tone: "bad" });
          window.location.hash = "";
        })
        .finally(() => setLoading(false));
    };
    load();
    window.addEventListener("hashchange", load);
    return () => window.removeEventListener("hashchange", load);
  }, []);

  const startGame = async () => {
    setBusy(true);
    try {
      const g = await api.createGame(name.trim());
      setGame(g);
      setSelected([]);
      setMessage({
        text: "Roll to begin. Make 1 to clear the first card.",
        tone: "info",
      });
      window.history.replaceState(null, "", `#/games/${g.id}`);
    } catch (e) {
      setMessage({
        text: e instanceof Error ? e.message : String(e),
        tone: "bad",
      });
    } finally {
      setBusy(false);
    }
  };

  const send = useCallback(
    async (move: Move) => {
      if (!game) return;
      setBusy(true);
      const animate = move.type === "roll";
      if (animate) setRolling(true);
      try {
        const [next] = await Promise.all([
          api.move(game.id, move),
          animate ? new Promise((r) => setTimeout(r, ROLL_ANIMATION_MS)) : null,
        ]);
        setSelected([]);
        setMessage(describe(game, next, move));
        setGame(next);
      } catch (e) {
        const text =
          e instanceof ApiError || e instanceof Error ? e.message : String(e);
        setMessage({ text, tone: "bad" });
      } finally {
        setRolling(false);
        setBusy(false);
      }
    },
    [game],
  );

  if (loading)
    return (
      <main className="shell">
        <p className="muted">Loading…</p>
      </main>
    );

  if (!game) {
    return (
      <main className="shell start">
        <h1 className="logo">
          SimpleSwitch<span>16</span>
        </h1>
        <p>
          Inspired by Switch 16, a classic 2-to-4-player family board game
          published by Tomy and designed by Anthony Vadasz
        </p>
        <p className="lede">
          Roll the dice. Pick dice that add up to your card. Clear cards 1
          through 16 in as few rolls as you can.
        </p>
        <form
          className="start-form"
          onSubmit={(e) => {
            e.preventDefault();
            void startGame();
          }}
        >
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Your name (optional)"
            maxLength={32}
            aria-label="Your name"
          />
          <button className="primary" disabled={busy}>
            New game
          </button>
        </form>
        {message && <p className={`message ${message.tone}`}>{message.text}</p>}
        <Rules />
      </main>
    );
  }

  const player = game.players[game.current_player];
  const won = game.status.state === "won";
  const claiming = game.board.phase === "claiming" && !won;
  const selectedSum = selected.reduce(
    (s, i) => s + game.board.dice[i].value,
    0,
  );
  const target = player.card;

  const toggle = (i: number) =>
    setSelected((s) => (s.includes(i) ? s.filter((x) => x !== i) : [...s, i]));

  return (
    <main className="shell">
      <header className="topbar">
        <h1 className="logo small">
          SimpleSwitch<span>16</span>
        </h1>
        <div className="stats">
          <span>{player.name}</span>
          <span className="pill">
            Rolls <strong>{player.rolls}</strong>
          </span>
        </div>
      </header>

      <CardTrack current={target} won={won} />

      <section className="table">
        <div className={`card ${won ? "done" : ""}`} aria-live="polite">
          <span className="card-label">{won ? "Cleared" : "Make"}</span>
          <span className="card-value">{won ? 16 : target}</span>
        </div>

        <div className="tray">
          <div className="dice" role="group" aria-label="Dice">
            {game.board.dice.length === 0 && !rolling && (
              <p className="muted">Roll {game.next_roll_dice} dice</p>
            )}
            {(rolling
              ? Array.from({ length: game.next_roll_dice }, (_, i) => ({
                  value: (i % 6) + 1,
                  used: false,
                }))
              : game.board.dice
            ).map((d, i) => (
              <DieFace
                key={i}
                value={d.value}
                used={d.used}
                selected={selected.includes(i)}
                disabled={!claiming || busy}
                rolling={rolling}
                delay={i * 60}
                onToggle={() => toggle(i)}
              />
            ))}
          </div>

          {claiming && (
            <p
              className={`sum ${selectedSum === target ? "match" : selectedSum > target ? "over" : ""}`}
            >
              Selected: <strong>{selectedSum}</strong> / {target}
            </p>
          )}
        </div>

        <div className="actions">
          {!won && !claiming && (
            <button
              className="primary"
              disabled={busy}
              onClick={() => void send({ type: "roll" })}
            >
              Roll {game.next_roll_dice} dice
            </button>
          )}
          {claiming && (
            <>
              <button
                className="primary"
                disabled={busy || selectedSum !== target}
                onClick={() => void send({ type: "claim", dice: selected })}
              >
                Claim {target}
              </button>
              <button
                className="ghost"
                disabled={busy}
                onClick={() => void send({ type: "pass" })}
              >
                Pass
              </button>
            </>
          )}
        </div>

        {message && <p className={`message ${message.tone}`}>{message.text}</p>}
      </section>

      {won && game.status.state === "won" && (
        <div className="overlay" role="dialog" aria-labelledby="won-title">
          <div className="modal">
            <h2 id="won-title">You cleared all 16 cards!</h2>
            <p className="big-number">{game.status.rolls}</p>
            <p>{game.status.rolls === 1 ? "roll" : "rolls"} to win</p>
            <button
              className="primary"
              onClick={() => {
                window.history.replaceState(null, "", "#");
                setGame(null);
                setMessage(null);
              }}
            >
              Play again
            </button>
          </div>
        </div>
      )}
    </main>
  );
}

function describe(
  prev: Game,
  next: Game,
  move: Move,
): { text: string; tone: "info" | "good" | "bad" } {
  const card = next.players[next.current_player].card;
  const prevCard = prev.players[prev.current_player].card;
  if (next.status.state === "won")
    return { text: "Card 16 cleared!", tone: "good" };
  if (next.board.last_turn_end === "passed")
    return { text: `Passed. Roll again for ${card}.`, tone: "info" };
  if (next.board.last_turn_end === "no_combination") {
    const cleared = card - prevCard;
    const lead =
      cleared > 0
        ? `Cleared ${cleared === 1 ? `card ${prevCard}` : `${cleared} cards`}. `
        : "";
    return {
      text: `${lead}No way to make ${card} — turn over. Roll again.`,
      tone: cleared > 0 ? "good" : "info",
    };
  }
  if (move.type === "claim")
    return {
      text: `Card ${prevCard} cleared! Now make ${card}.`,
      tone: "good",
    };
  return { text: `Pick dice that add up to ${card}.`, tone: "info" };
}

function Rules() {
  return (
    <details className="rules">
      <summary>How to play</summary>
      <ul>
        <li>
          Your card starts at 1. Roll the dice, then select dice that add up to
          your card.
        </li>
        <li>
          Each claim clears the card. Keep going with all the dice you rolled.
        </li>
        <li>
          When the dice can’t make your card, your turn ends — roll again.
        </li>
        <li>
          You roll 3 dice for cards 1–6, 4 dice for 7–11 and 5 dice for 12–16.
        </li>
        <li>Clear card 16 to win. Fewest rolls is best.</li>
      </ul>
    </details>
  );
}
