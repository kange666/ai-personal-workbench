export const TOWER_WIDTH = 480
export const TOWER_HEIGHT = 340
export const TOWER_BLOCK_HEIGHT = 20
export const TOWER_BASE_Y = 300
// 包括底座在内的存档上限，达到后作为正常通关保留完整高塔。
export const TOWER_MAX_BLOCKS = 10000

export type TowerBlock = { x: number; width: number }
export type TowerState = {
  blocks: TowerBlock[]
  movingX: number
  direction: 1 | -1
  score: number
  perfectStreak: number
  lastPerfect: boolean
  phase: 'ready' | 'moving' | 'over' | 'complete'
}

export function createTower(): TowerState {
  return { blocks: [{ x: 150, width: 180 }], movingX: 0, direction: 1, score: 0, perfectStreak: 0, lastPerfect: false, phase: 'ready' }
}

export function isTowerState(value: unknown): value is TowerState {
  if (!value || typeof value !== 'object') return false
  const state = value as TowerState
  if (!Array.isArray(state.blocks) || state.blocks.length < 1 || state.blocks.length > TOWER_MAX_BLOCKS) return false
  if (state.phase === 'complete' && state.blocks.length !== TOWER_MAX_BLOCKS) return false
  let previous: TowerBlock | undefined
  for (const block of state.blocks) {
    if (!block || typeof block.x !== 'number' || typeof block.width !== 'number' || !Number.isFinite(block.x) || !Number.isFinite(block.width) || block.x < 0 || block.width <= 0 || block.x + block.width > TOWER_WIDTH) return false
    if (previous && (block.x < previous.x - 0.00001 || block.x + block.width > previous.x + previous.width + 0.00001)) return false
    previous = block
  }
  if (state.blocks[0]!.x !== 150 || state.blocks[0]!.width !== 180) return false
  const height = state.blocks.length - 1
  if (!Number.isSafeInteger(state.score) || state.score < height * 10 || state.score > height * 35) return false
  if (!Number.isInteger(state.perfectStreak) || state.perfectStreak < 0 || state.perfectStreak > height || typeof state.lastPerfect !== 'boolean' || state.lastPerfect !== (state.perfectStreak > 0)) return false
  return typeof state.movingX === 'number' && Number.isFinite(state.movingX) && state.movingX >= 0 && state.movingX <= TOWER_WIDTH - previous!.width && (state.direction === 1 || state.direction === -1) && ['ready', 'moving', 'over', 'complete'].includes(state.phase)
}

export function startTower(state: TowerState) {
  if (state.phase === 'ready') state.phase = state.blocks.length >= TOWER_MAX_BLOCKS ? 'complete' : 'moving'
}

export function stepTower(state: TowerState, seconds: number) {
  if (state.phase !== 'moving' || !Number.isFinite(seconds) || seconds <= 0) return
  if (state.blocks.length >= TOWER_MAX_BLOCKS) { state.phase = 'complete'; return }
  const speed = Math.min(340, 115 + (state.blocks.length - 1) * 7)
  const end = TOWER_WIDTH - state.blocks[state.blocks.length - 1]!.width
  state.movingX += state.direction * speed * Math.min(seconds, 0.05)
  if (state.movingX > end) { state.movingX = end - (state.movingX - end); state.direction = -1 }
  if (state.movingX < 0) { state.movingX = -state.movingX; state.direction = 1 }
}

export type TowerDrop = { kind: 'perfect' | 'trimmed' | 'miss'; cut: TowerBlock | null }

export function dropTower(state: TowerState): TowerDrop | null {
  if (state.phase !== 'moving') return null
  if (state.blocks.length >= TOWER_MAX_BLOCKS) { state.phase = 'complete'; return null }
  const previous = state.blocks[state.blocks.length - 1]!
  const delta = state.movingX - previous.x
  const tolerance = Math.min(4, previous.width * 0.08)
  const perfect = Math.abs(delta) <= tolerance
  const left = perfect ? previous.x : Math.max(previous.x, state.movingX)
  const right = perfect ? previous.x + previous.width : Math.min(previous.x + previous.width, state.movingX + previous.width)
  if (right - left <= 0) {
    state.phase = 'over'
    return { kind: 'miss', cut: { x: state.movingX, width: previous.width } }
  }
  const cut = perfect ? null : delta > 0 ? { x: right, width: delta } : { x: state.movingX, width: -delta }
  state.blocks.push({ x: left, width: right - left })
  state.lastPerfect = perfect
  state.perfectStreak = perfect ? state.perfectStreak + 1 : 0
  state.score += 10 + (perfect ? 5 * Math.min(state.perfectStreak, 5) : 0)
  state.direction = state.blocks.length % 2 === 0 ? -1 : 1
  state.movingX = state.direction === 1 ? 0 : TOWER_WIDTH - (right - left)
  if (state.blocks.length === TOWER_MAX_BLOCKS) state.phase = 'complete'
  return { kind: perfect ? 'perfect' : 'trimmed', cut }
}
