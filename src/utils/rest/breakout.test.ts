import { describe, expect, it } from 'vitest'
import { BALL_RADIUS, BREAKOUT_WIDTH, PADDLE_Y, brickRect, createBreakout, isBreakoutState, launchBreakout, moveBreakoutPaddle, stepBreakout } from './breakout'

describe('打砖块物理与存档', () => {
  it('初始存档有效，只有发球后球才运动', () => {
    const state = createBreakout()
    expect(isBreakoutState(state)).toBe(true)
    const initialBall = { ...state.ball }
    stepBreakout(state, 0.05)
    expect(state.ball).toEqual(initialBall)
    launchBreakout(state)
    stepBreakout(state, 0.05)
    expect(state.ball.y).toBeLessThan(initialBall.y)
    expect(isBreakoutState(state)).toBe(true)
  })

  it('挡板限制在舞台内，准备阶段小球随挡板移动', () => {
    const state = createBreakout()
    moveBreakoutPaddle(state, -100)
    expect(state.paddleX).toBe(41)
    expect(state.ball.x).toBe(41)
    moveBreakoutPaddle(state, 1000)
    expect(state.paddleX).toBe(BREAKOUT_WIDTH - 41)
    moveBreakoutPaddle(state, Number.NaN)
    expect(isBreakoutState(state)).toBe(true)
  })

  it('高速小球不会穿过砖块，每次接触只计一次命中', () => {
    const state = createBreakout()
    state.level = 2
    state.bricks[21] = 1
    const brick = brickRect(21)
    state.phase = 'playing'
    state.ball = { x: brick.x + brick.width / 2, y: brick.y + brick.height + BALL_RADIUS + 3, vx: 0, vy: -430 }
    stepBreakout(state, 0.05)
    expect(state.bricks[21]).toBe(0)
    expect(state.ball.vy).toBeGreaterThan(0)
    expect(state.score).toBe(10)
    stepBreakout(state, 0.05)
    expect(state.score).toBe(10)
  })

  it('擦过厚砖角落后离开，不会在接触边缘重复计分', () => {
    const state = createBreakout()
    state.level = 2
    state.bricks = Array(28).fill(0)
    state.bricks[0] = 2
    state.bricks[27] = 1
    const brick = brickRect(0)
    state.phase = 'playing'
    state.ball = { x: brick.x - 4, y: brick.y + brick.height + BALL_RADIUS + 2, vx: 0, vy: -240 }
    stepBreakout(state, 0.05)
    expect(state.score).toBe(10)
    expect(state.bricks[0]).toBe(1)
    stepBreakout(state, 0.05)
    expect(state.score).toBe(10)
    expect(isBreakoutState(state)).toBe(true)
  })

  it('挡板边缘改变反弹角度且保持向上运动', () => {
    const state = createBreakout()
    state.phase = 'playing'
    state.ball = { x: state.paddleX + 32, y: PADDLE_Y - BALL_RADIUS - 2, vx: 0, vy: 240 }
    stepBreakout(state, 0.05)
    expect(state.ball.vy).toBeLessThan(0)
    expect(state.ball.vx).toBeGreaterThan(0)
    expect(state.lives).toBe(3)
  })

  it('左右墙与顶部反弹后保持有效坐标', () => {
    const state = createBreakout()
    state.phase = 'playing'
    state.ball = { x: BALL_RADIUS + 1, y: BALL_RADIUS + 1, vx: -170, vy: -170 }
    stepBreakout(state, 0.05)
    expect(state.ball.vx).toBeGreaterThan(0)
    expect(state.ball.vy).toBeGreaterThan(0)
    expect(isBreakoutState(state)).toBe(true)
  })

  it('漏球只损失一条生命，最后一条结束并阻止继续推进', () => {
    const state = createBreakout()
    state.phase = 'playing'
    state.ball = { x: 20, y: 324, vx: 0, vy: 240 }
    stepBreakout(state, 0.05)
    expect(state.lives).toBe(2)
    expect(state.phase).toBe('ready')
    state.lives = 1
    state.phase = 'playing'
    state.ball = { x: 20, y: 324, vx: 0, vy: 240 }
    stepBreakout(state, 0.05)
    expect(state.lives).toBe(0)
    expect(state.phase).toBe('over')
    expect(isBreakoutState(state)).toBe(true)
    const snapshot = JSON.stringify(state)
    stepBreakout(state, 0.05, 1)
    launchBreakout(state)
    expect(JSON.stringify(state)).toBe(snapshot)
  })

  it('清空最后一块进入下一关，等待再次发球', () => {
    const state = createBreakout()
    state.bricks = Array(28).fill(0)
    state.bricks[21] = 1
    const brick = brickRect(21)
    state.phase = 'playing'
    state.ball = { x: brick.x + 28, y: brick.y + brick.height + BALL_RADIUS + 2, vx: 0, vy: -240 }
    stepBreakout(state, 0.05)
    expect(state.level).toBe(2)
    expect(state.score).toBe(110)
    expect(state.phase).toBe('ready')
    expect(state.bricks[0]).toBe(2)
    expect(isBreakoutState(state)).toBe(true)
  })

  it('拒绝坏存档和不合理的生命、速度及砖块', () => {
    for (const value of [null, {}, { ...createBreakout(), lives: 0 }, { ...createBreakout(), score: -1 }, { ...createBreakout(), bricks: [1] }, { ...createBreakout(), ball: { x: 1, y: 1, vx: Infinity, vy: 0 } }, { ...createBreakout(), phase: 'playing' }]) expect(isBreakoutState(value)).toBe(false)
  })
})
