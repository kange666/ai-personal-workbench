export const BREAKOUT_WIDTH = 480
export const BREAKOUT_HEIGHT = 320
export const BALL_RADIUS = 6
export const PADDLE_WIDTH = 82
export const PADDLE_Y = 287
export const BRICK_COLUMNS = 7
export const BRICK_ROWS = 4

export type BreakoutState = {
  level: number
  lives: number
  score: number
  phase: 'ready' | 'playing' | 'over'
  paddleX: number
  ball: { x: number; y: number; vx: number; vy: number }
  bricks: number[]
}

export function brickRect(index: number) {
  return { x: 26 + (index % BRICK_COLUMNS) * 62, y: 38 + Math.floor(index / BRICK_COLUMNS) * 24, width: 56, height: 17 }
}

function newBricks(level: number) {
  return Array.from({ length: BRICK_COLUMNS * BRICK_ROWS }, (_, i) => level > 1 && i < BRICK_COLUMNS ? 2 : 1)
}

export function prepareBreakoutBall(state: BreakoutState) {
  state.ball = { x: state.paddleX, y: PADDLE_Y - BALL_RADIUS - 1, vx: 0, vy: 0 }
}

export function createBreakout(): BreakoutState {
  const state: BreakoutState = { level: 1, lives: 3, score: 0, phase: 'ready', paddleX: BREAKOUT_WIDTH / 2, ball: { x: 0, y: 0, vx: 0, vy: 0 }, bricks: newBricks(1) }
  prepareBreakoutBall(state)
  return state
}

export function isBreakoutState(value: unknown): value is BreakoutState {
  if (!value || typeof value !== 'object') return false
  const state = value as BreakoutState
  const numberIn = (n: unknown, min: number, max: number) => typeof n === 'number' && Number.isFinite(n) && n >= min && n <= max
  if (!Number.isSafeInteger(state.level) || !numberIn(state.level, 1, 100000) || !Number.isSafeInteger(state.score) || !numberIn(state.score, 0, 1000000000)) return false
  if (!Number.isInteger(state.lives) || !numberIn(state.lives, 0, 3) || !['ready', 'playing', 'over'].includes(state.phase)) return false
  if ((state.lives === 0) !== (state.phase === 'over') || !numberIn(state.paddleX, PADDLE_WIDTH / 2, BREAKOUT_WIDTH - PADDLE_WIDTH / 2)) return false
  if (!state.ball || !numberIn(state.ball.x, BALL_RADIUS, BREAKOUT_WIDTH - BALL_RADIUS) || !numberIn(state.ball.y, BALL_RADIUS, BREAKOUT_HEIGHT + BALL_RADIUS)) return false
  if (!numberIn(state.ball.vx, -500, 500) || !numberIn(state.ball.vy, -500, 500)) return false
  if (state.phase === 'playing' && (Math.abs(state.ball.vy) < 20 || Math.hypot(state.ball.vx, state.ball.vy) > 501)) return false
  return Array.isArray(state.bricks) && state.bricks.length === BRICK_COLUMNS * BRICK_ROWS && state.bricks.some(hp => hp > 0) && state.bricks.every((hp, i) => Number.isInteger(hp) && hp >= 0 && hp <= (state.level > 1 && i < BRICK_COLUMNS ? 2 : 1))
}

export function moveBreakoutPaddle(state: BreakoutState, center: number) {
  if (state.phase === 'over' || !Number.isFinite(center)) return
  state.paddleX = Math.max(PADDLE_WIDTH / 2, Math.min(BREAKOUT_WIDTH - PADDLE_WIDTH / 2, center))
  if (state.phase === 'ready') prepareBreakoutBall(state)
}

export function launchBreakout(state: BreakoutState) {
  if (state.phase !== 'ready') return
  const speed = Math.min(430, 220 + state.level * 20)
  state.ball.vx = speed * 0.36
  state.ball.vy = -speed * Math.sqrt(1 - 0.36 ** 2)
  state.phase = 'playing'
}

/** 每次位移最多半个球半径，避免低帧率和后期高速时跳过砖块。 */
export function stepBreakout(state: BreakoutState, seconds: number, direction = 0) {
  if (state.phase === 'over' || !Number.isFinite(seconds) || seconds <= 0) return
  const dt = Math.min(seconds, 0.05)
  if (direction) moveBreakoutPaddle(state, state.paddleX + Math.sign(direction) * 410 * dt)
  if (state.phase !== 'playing') return
  const steps = Math.max(1, Math.ceil(Math.hypot(state.ball.vx, state.ball.vy) * dt / (BALL_RADIUS / 2)))
  const slice = dt / steps
  for (let step = 0; step < steps && state.phase === 'playing'; step++) {
    const ball = state.ball
    const previousX = ball.x
    const previousY = ball.y
    ball.x += ball.vx * slice
    ball.y += ball.vy * slice
    if (ball.x < BALL_RADIUS) { ball.x = BALL_RADIUS; ball.vx = Math.abs(ball.vx) }
    if (ball.x > BREAKOUT_WIDTH - BALL_RADIUS) { ball.x = BREAKOUT_WIDTH - BALL_RADIUS; ball.vx = -Math.abs(ball.vx) }
    if (ball.y < BALL_RADIUS) { ball.y = BALL_RADIUS; ball.vy = Math.abs(ball.vy) }

    const paddleLeft = state.paddleX - PADDLE_WIDTH / 2
    if (ball.vy > 0 && previousY + BALL_RADIUS <= PADDLE_Y && ball.y + BALL_RADIUS >= PADDLE_Y && ball.x >= paddleLeft - BALL_RADIUS && ball.x <= paddleLeft + PADDLE_WIDTH + BALL_RADIUS) {
      const offset = Math.max(-1, Math.min(1, (ball.x - state.paddleX) / (PADDLE_WIDTH / 2)))
      const speed = Math.hypot(ball.vx, ball.vy)
      const angle = offset * Math.PI / 3
      ball.x = Math.max(BALL_RADIUS, Math.min(BREAKOUT_WIDTH - BALL_RADIUS, ball.x))
      ball.y = PADDLE_Y - BALL_RADIUS
      ball.vx = speed * Math.sin(angle)
      ball.vy = -speed * Math.cos(angle)
    }

    for (let i = 0; i < state.bricks.length; i++) {
      if (!state.bricks[i]) continue
      const brick = brickRect(i)
      const closestX = Math.max(brick.x, Math.min(brick.x + brick.width, ball.x))
      const closestY = Math.max(brick.y, Math.min(brick.y + brick.height, ball.y))
      if ((ball.x - closestX) ** 2 + (ball.y - closestY) ** 2 > BALL_RADIUS ** 2) continue
      const distance = Math.hypot(ball.x - closestX, ball.y - closestY)
      let normalX = distance > 0 ? (ball.x - closestX) / distance : 0
      let normalY = distance > 0 ? (ball.y - closestY) / distance : 0
      if (distance === 0) {
        if (previousY <= brick.y) normalY = -1
        else if (previousY >= brick.y + brick.height) normalY = 1
        else normalX = previousX < brick.x + brick.width / 2 ? -1 : 1
      }
      const toward = ball.vx * normalX + ball.vy * normalY
      // 擦过边缘或已经离开的球不重复扣血。
      if (toward >= 0) continue
      state.bricks[i] = state.bricks[i]! - 1
      state.score += 10
      ball.x = closestX + normalX * (BALL_RADIUS + 0.01)
      ball.y = closestY + normalY * (BALL_RADIUS + 0.01)
      ball.vx -= 2 * toward * normalX
      ball.vy -= 2 * toward * normalY
      if (Math.abs(ball.vy) < 50) {
        const speed = Math.hypot(ball.vx, ball.vy)
        ball.vy = Math.sign(ball.vy || 1) * Math.min(50, speed)
        ball.vx = Math.sign(ball.vx || 1) * Math.sqrt(Math.max(0, speed ** 2 - ball.vy ** 2))
      }
      if (state.bricks.every(hp => hp === 0)) {
        state.level++
        state.score += 100
        state.bricks = newBricks(state.level)
        state.phase = 'ready'
        prepareBreakoutBall(state)
      }
      break
    }
    if (ball.y > BREAKOUT_HEIGHT + BALL_RADIUS) {
      state.lives--
      state.phase = state.lives > 0 ? 'ready' : 'over'
      prepareBreakoutBall(state)
    }
  }
}
