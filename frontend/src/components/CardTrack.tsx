const CARDS = Array.from({ length: 16 }, (_, i) => i + 1)

function diceFor(card: number) {
  return card <= 6 ? 3 : card <= 11 ? 4 : 5
}

/** The 16 cards in a row: cleared, current and upcoming, grouped by dice count. */
export function CardTrack({ current, won }: { current: number; won: boolean }) {
  return (
    <ol className="track" aria-label="Card progress">
      {CARDS.map((c) => {
        const state = won || c < current ? 'cleared' : c === current ? 'current' : 'todo'
        const tierStart = c === 1 || c === 7 || c === 12
        return (
          <li key={c} className={`track-card ${state} ${tierStart ? 'tier-start' : ''}`}>
            {tierStart && <span className="tier-label">{diceFor(c)} dice</span>}
            {c}
          </li>
        )
      })}
    </ol>
  )
}
