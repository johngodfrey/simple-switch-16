const PIPS: Record<number, [number, number][]> = {
  1: [[50, 50]],
  2: [[28, 28], [72, 72]],
  3: [[28, 28], [50, 50], [72, 72]],
  4: [[28, 28], [72, 28], [28, 72], [72, 72]],
  5: [[28, 28], [72, 28], [50, 50], [28, 72], [72, 72]],
  6: [[28, 26], [72, 26], [28, 50], [72, 50], [28, 74], [72, 74]],
}

interface Props {
  value: number
  used: boolean
  selected: boolean
  disabled: boolean
  rolling: boolean
  delay: number
  onToggle: () => void
}

export function DieFace({ value, used, selected, disabled, rolling, delay, onToggle }: Props) {
  const cls = ['die', used && 'used', selected && 'selected', rolling && 'rolling']
    .filter(Boolean)
    .join(' ')
  return (
    <button
      type="button"
      className={cls}
      style={{ animationDelay: `${delay}ms` }}
      disabled={disabled || used}
      aria-pressed={selected}
      aria-label={`Die showing ${value}${used ? ', used' : ''}`}
      onClick={onToggle}
    >
      <svg viewBox="0 0 100 100" aria-hidden="true">
        <rect x="4" y="4" width="92" height="92" rx="18" className="die-body" />
        {PIPS[value]?.map(([cx, cy], i) => (
          <circle key={i} cx={cx} cy={cy} r="9" className="pip" />
        ))}
      </svg>
    </button>
  )
}
