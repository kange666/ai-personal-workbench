import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import BreakoutGame from './BreakoutGame.vue'
import { gameSaveKey } from '../../composables/useGameSave'
import { createBreakout, launchBreakout } from '../../utils/rest/breakout'

const loop = vi.hoisted(() => ({ paused: (): boolean => false, tick: (_dt: number) => {} }))
vi.mock('../../composables/useGameLoop', () => ({ useGameLoop: (paused: () => boolean, tick: (dt: number) => void) => { loop.paused = paused; loop.tick = tick } }))

describe('打砖块操作', () => {
  beforeEach(() => localStorage.clear())

  it('外部方向键不移动挡板，舞台内方向键有效，暂停禁用发球', async () => {
    const wrapper = mount(BreakoutGame, { props: { paused: false } })
    const initialX = wrapper.get('.paddle').attributes('x')
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }))
    loop.tick(0.05)
    await nextTick()
    expect(wrapper.get('.paddle').attributes('x')).toBe(initialX)
    await wrapper.get('[data-game-focus]').trigger('keydown', { key: 'ArrowRight' })
    loop.tick(0.05)
    await nextTick()
    expect(Number(wrapper.get('.paddle').attributes('x'))).toBeGreaterThan(Number(initialX))
    await wrapper.setProps({ paused: true })
    await wrapper.get('[data-game-focus]').trigger('keydown', { key: ' ' })
    expect(wrapper.text()).toContain('准备好，发球吧')
    expect(wrapper.get('button').attributes('disabled')).toBeDefined()
    expect(loop.paused()).toBe(true)
    wrapper.unmount()
  })

  it('恢复飞行中存档先冻结球的原位置与速度，重开保留最高分', async () => {
    const state = createBreakout()
    state.score = 80
    state.bricks[0] = 0
    launchBreakout(state)
    state.ball.x = 120
    state.ball.y = 210
    localStorage.setItem(gameSaveKey('breakout'), JSON.stringify({ version: 1, state, best: 100 }))
    const wrapper = mount(BreakoutGame, { props: { paused: false } })
    expect(wrapper.text()).toContain('继续刚才的这一球')
    expect(loop.paused()).toBe(true)
    expect(wrapper.get('.ball').attributes('cx')).toBe('120')
    expect(wrapper.get('.ball').attributes('cy')).toBe('210')
    expect(wrapper.findAll('.brick')).toHaveLength(27)
    await wrapper.get('button').trigger('click')
    expect(wrapper.text()).toContain('小球飞行中')
    expect(loop.paused()).toBe(false)
    const resumed = JSON.parse(localStorage.getItem(gameSaveKey('breakout'))!).state
    expect(resumed.ball).toEqual(state.ball)
    ;(wrapper.vm as unknown as { restart: () => void }).restart()
    await nextTick()
    expect(wrapper.findAll('.brick')).toHaveLength(28)
    expect(wrapper.get('.game-stats').text()).toContain('最高 100')
    wrapper.unmount()
  })

  it('失焦释放方向键，回到游戏不会继续滑动', async () => {
    const wrapper = mount(BreakoutGame, { props: { paused: false } })
    await wrapper.get('[data-game-focus]').trigger('keydown', { key: 'ArrowLeft' })
    window.dispatchEvent(new Event('blur'))
    loop.tick(0.05)
    await nextTick()
    expect(wrapper.get('.paddle').attributes('x')).toBe('199')
    wrapper.unmount()
  })

  it('高帧率命中不逐帧写盘，立刻关闭页面仍保存最新最高分', async () => {
    const state = createBreakout()
    state.phase = 'playing'
    state.ball = { x: 54, y: 135, vx: 0, vy: -240 }
    localStorage.setItem(gameSaveKey('breakout'), JSON.stringify({ version: 1, state, best: 0 }))
    const wrapper = mount(BreakoutGame, { props: { paused: false } })
    await wrapper.get('button').trigger('click')
    const writes = vi.spyOn(Storage.prototype, 'setItem')
    for (let i = 0; i < 60; i++) loop.tick(1 / 240)
    await nextTick()
    expect(wrapper.get('.game-stats').text()).toContain('最高 10')
    expect(writes).not.toHaveBeenCalled()
    window.dispatchEvent(new Event('pagehide'))
    expect(JSON.parse(localStorage.getItem(gameSaveKey('breakout'))!).best).toBe(10)
    writes.mockRestore()
    wrapper.unmount()
  })
})
