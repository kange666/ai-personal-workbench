import { describe, expect, it } from 'vitest'
import { TOWER_MAX_BLOCKS, createTower, dropTower, isTowerState, startTower, stepTower } from './tower'

describe('堆塔裁切与存档', () => {
  it('初始等待开始，准备期间不移动也不能放块', () => {
    const state = createTower()
    expect(isTowerState(state)).toBe(true)
    stepTower(state, 0.05)
    expect(state.movingX).toBe(0)
    expect(dropTower(state)).toBeNull()
    startTower(state)
    stepTower(state, 0.05)
    expect(state.movingX).toBeGreaterThan(0)
  })

  it('向右超出部分裁掉，下一块沿用剩余宽度', () => {
    const state = createTower()
    startTower(state)
    state.movingX = 180
    expect(dropTower(state)).toEqual({ kind: 'trimmed', cut: { x: 330, width: 30 } })
    expect(state.blocks[1]).toEqual({ x: 180, width: 150 })
    expect(state.movingX).toBe(330)
    expect(state.direction).toBe(-1)
    expect(state.score).toBe(10)
    expect(isTowerState(state)).toBe(true)
  })

  it('向左超出部分裁掉，不会把支撑范围向外扩张', () => {
    const state = createTower()
    startTower(state)
    state.movingX = 100
    expect(dropTower(state)).toEqual({ kind: 'trimmed', cut: { x: 100, width: 50 } })
    expect(state.blocks[1]).toEqual({ x: 150, width: 130 })
    expect(isTowerState(state)).toBe(true)
  })

  it('完美容差对齐，连续对齐增加奖励', () => {
    const state = createTower()
    startTower(state)
    state.movingX = 153
    expect(dropTower(state)?.kind).toBe('perfect')
    expect(state.blocks[1]).toEqual({ x: 150, width: 180 })
    expect(state.score).toBe(15)
    state.movingX = 149
    expect(dropTower(state)?.kind).toBe('perfect')
    expect(state.perfectStreak).toBe(2)
    expect(state.score).toBe(35)
    expect(isTowerState(state)).toBe(true)
  })

  it('刚好擦边没有重叠则结束，不添加假方块', () => {
    const state = createTower()
    startTower(state)
    state.movingX = 250
    dropTower(state)
    state.movingX = 170
    expect(dropTower(state)?.kind).toBe('miss')
    expect(state.phase).toBe('over')
    expect(state.blocks).toHaveLength(2)
    const snapshot = JSON.stringify(state)
    stepTower(state, 0.05)
    dropTower(state)
    expect(JSON.stringify(state)).toBe(snapshot)
    expect(isTowerState(state)).toBe(true)
  })

  it('未接住后存档仍有效，重新开始恢复完整底座', () => {
    const state = createTower()
    startTower(state)
    state.movingX = 290
    dropTower(state)
    state.movingX = 0
    dropTower(state)
    expect(state.phase).toBe('over')
    expect(isTowerState(state)).toBe(true)
    expect(createTower().blocks[0]?.width).toBe(180)
  })

  it('边界反向且限制大时间步，窄方块仍可正常推进', () => {
    const state = createTower()
    startTower(state)
    state.movingX = 299
    stepTower(state, 10)
    expect(state.direction).toBe(-1)
    expect(state.movingX).toBeLessThan(300)
    state.movingX = 1
    stepTower(state, 0.05)
    expect(state.direction).toBe(1)
    expect(state.movingX).toBeGreaterThan(0)
    expect(isTowerState(state)).toBe(true)
  })

  it('拒绝非有限数字、越界方块与不一致的分数', () => {
    for (const value of [null, {}, { ...createTower(), movingX: NaN }, { ...createTower(), blocks: [{ x: 150, width: -1 }] }, { ...createTower(), blocks: [{ x: 150, width: 180 }, { x: 100, width: 180 }], score: 10 }, { ...createTower(), direction: 0 }, { ...createTower(), score: 10 }, { ...createTower(), lastPerfect: true }]) expect(isTowerState(value)).toBe(false)
  })

  it('达到最大高度正常通关，保留得分且序列化后可恢复', () => {
    const state = createTower()
    startTower(state)
    for (let i = 1; i < TOWER_MAX_BLOCKS; i++) {
      state.movingX = 150
      dropTower(state)
    }
    expect(state.blocks).toHaveLength(TOWER_MAX_BLOCKS)
    expect(state.phase).toBe('complete')
    expect(state.score).toBe((TOWER_MAX_BLOCKS - 1) * 35 - 50)
    const snapshot = JSON.stringify(state)
    expect(isTowerState(JSON.parse(snapshot))).toBe(true)
    stepTower(state, 0.05)
    startTower(state)
    expect(dropTower(state)).toBeNull()
    expect(JSON.stringify(state)).toBe(snapshot)
  })

  it('兼容旧存档的满高运行态，防止继续添加越界方块', () => {
    const state = createTower()
    state.blocks = Array.from({ length: TOWER_MAX_BLOCKS }, () => ({ x: 150, width: 180 }))
    state.score = (TOWER_MAX_BLOCKS - 1) * 35 - 50
    state.perfectStreak = TOWER_MAX_BLOCKS - 1
    state.lastPerfect = true
    state.phase = 'moving'
    state.movingX = 150
    expect(isTowerState(state)).toBe(true)
    expect(dropTower(state)).toBeNull()
    expect(state.blocks).toHaveLength(TOWER_MAX_BLOCKS)
    expect(state.phase).toBe('complete')
    expect(isTowerState(state)).toBe(true)
    expect(isTowerState({ ...createTower(), phase: 'complete' })).toBe(false)
  })
})
