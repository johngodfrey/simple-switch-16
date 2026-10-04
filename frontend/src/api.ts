// Types mirror the JSON produced by the Rust `switch16-core` + `switch16-api` crates.

export type TurnPhase = 'awaiting_roll' | 'claiming'
export type TurnEnd = 'no_combination' | 'passed'

export interface Die {
  value: number
  used: boolean
}

export interface Player {
  name: string
  card: number
  rolls: number
}

export type GameStatus =
  | { state: 'in_progress' }
  | { state: 'won'; winner: number; rolls: number }

export interface Game {
  id: string
  players: Player[]
  current_player: number
  board: {
    dice: Die[]
    phase: TurnPhase
    last_turn_end: TurnEnd | null
  }
  status: GameStatus
  can_claim: boolean
  next_roll_dice: number
}

export type Move =
  | { type: 'roll' }
  | { type: 'pass' }
  | { type: 'claim'; dice: number[] }

export class ApiError extends Error {
  readonly status: number
  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`/api${path}`, {
    ...init,
    headers: { 'content-type': 'application/json', ...init?.headers },
  })
  const body = await res.json().catch(() => ({}))
  if (!res.ok) throw new ApiError(res.status, body.error ?? res.statusText)
  return body as T
}

export const api = {
  createGame: (playerName?: string) =>
    request<Game>('/games', {
      method: 'POST',
      body: JSON.stringify({ player_name: playerName || undefined }),
    }),
  getGame: (id: string) => request<Game>(`/games/${encodeURIComponent(id)}`),
  move: (id: string, move: Move) =>
    request<Game>(`/games/${encodeURIComponent(id)}/moves`, {
      method: 'POST',
      body: JSON.stringify(move),
    }),
}
