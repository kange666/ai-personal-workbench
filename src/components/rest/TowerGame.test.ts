import { mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import TowerGame from './TowerGame.vue'
import { gameSaveKey } from '../../composables/useGameSave'
import { TOWER_MAX_BLOCKS, createTower, dropTower, startTower } from '../../utils/rest/tower'

vi.mock('../../composables/useGameLoop', () => ({ useGameLoop: vi.fn() }))

describe('堆塔挑战操作', () => {
  beforeEach(() => localStorage.clear())

  it('暂停时点击和键盘均不能开始，恢复后可以开始', async () => {
    const wrapper = mount(TowerGame, { props: { paused: true } })
    await wrapper.get('[data-game-focus]').trigger('pointerdown')
    await wrapper.get('[data-game-focus]').trigger('keydown', { key: ' ' })
    expect(wrapper.get('button').text()).toBe('开始搭建')
    await wrapper.setProps({ paused: false })
    await wrapper.get('[data-game-focus]').trigger('keydown', { key: ' ' })
    expect(wrapper.get('button').text()).toBe('放下方块')
    wrapper.unmount()
  })

  it('恢复存档先等待继续，连续按键不会误放方块', async () => {
    const state = createTower()
    startTower(state)
    state.movingX = 150
    dropTower(state)
    localStorage.setItem(gameSaveKey('tower'), JSON.stringify({ version: 1, state, best: 55 }))
    const wrapper = mount(TowerGame, { props: { paused: false } })
    expect(wrapper.get('button').text()).toBe('继续搭建')
    expect(wrapper.findAll('.tower-block')).toHaveLength(2)
    await wrapper.get('button').trigger('click')
    await wrapper.get('[data-game-focus]').trigger('keydown', { key: ' ', repeat: true })
    expect(wrapper.get('button').text()).toBe('放下方块')
    expect(wrapper.get('.game-stats').text()).toContain('最高 55')
    wrapper.unmount()
  })

  it('满高存档恢复显示完成，只渲染视口中的顶部层并保留完整成绩', async () => {
    const state = createTower()
    state.blocks = Array.from({ length: TOWER_MAX_BLOCKS }, () => ({ x: 150, width: 180 }))
    state.score = (TOWER_MAX_BLOCKS - 1) * 35 - 50
    state.perfectStreak = TOWER_MAX_BLOCKS - 1
    state.lastPerfect = true
    state.phase = 'complete'
    localStorage.setItem(gameSaveKey('tower'), JSON.stringify({ version: 1, state, best: state.score }))
    const wrapper = mount(TowerGame, { props: { paused: false } })
    expect(wrapper.text()).toContain('高塔已建成')
    expect(wrapper.text()).toContain('挑战完成，完整成绩已保留')
    expect(wrapper.findAll('.tower-block')).toHaveLength(9)
    expect(wrapper.find('.moving-block').exists()).toBe(false)
    expect(wrapper.findAll('.tower-block').at(-1)?.classes()).toContain(`block-tone-${(TOWER_MAX_BLOCKS - 1) % 3}`)
    expect(wrapper.findAll('.tower-block rect').at(-1)?.attributes('y')).toBe('160')
    expect(wrapper.get('.game-stats').text()).toContain(String(state.score))
    await wrapper.get('button').trigger('click')
    expect(wrapper.findAll('.tower-block')).toHaveLength(1)
    expect(wrapper.get('.game-stats').text()).toContain(`最高 ${state.score}`)
    wrapper.unmount()
  })
})
