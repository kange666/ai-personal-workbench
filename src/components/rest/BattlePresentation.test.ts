import { mount, type VueWrapper } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { gameSaveKey } from '../../composables/useGameSave';
import { addBattleEffect, BATTLE_EFFECT_LIMIT, stepBattleEffects, type BattleEffect } from '../../utils/rest/battleEffects';
import { createArchery, stepArchery } from '../../utils/rest/archery';
import { createDefense, createDefenseMeta, startDefense, validDefense, type EnemyKind } from '../../utils/rest/defense';
import ArcheryGame from './ArcheryGame.vue';
import DefenseGame from './DefenseGame.vue';

const loop = vi.hoisted(() => ({ tick: (_dt: number) => {}, paused: (): boolean => true }));
vi.mock('../../composables/useGameLoop', () => ({ useGameLoop: (paused: () => boolean, tick: (dt: number) => void) => { loop.paused = paused; loop.tick = tick; } }));
let wrapper: VueWrapper | undefined;
const store = (id: string, state: unknown) => localStorage.setItem(gameSaveKey(id), JSON.stringify({ version: 1, state, best: 0 }));
const read = (id: string) => JSON.parse(localStorage.getItem(gameSaveKey(id)) ?? 'null');
const button = (name: string) => wrapper!.findAll('button').find(b => b.text().includes(name))!;
async function advance(frames: number) { for (let i = 0; i < frames; i++) if (!loop.paused()) loop.tick(.05); await nextTick(); }
async function pointer(type: string, pointerId: number, clientX = 0, clientY = 0) {
  const event = new MouseEvent(type, { bubbles: true, button: 0, clientX, clientY });
  Object.defineProperty(event, 'pointerId', { value: pointerId });
  wrapper!.get('.archery-surface').element.dispatchEvent(event); await nextTick();
}
function enemy(id: number, kind: EnemyKind, hp = 100) {
  return { id, kind, x: 390, y: 200, hp, maxHP: hp, speed: 0, damage: 3, shield: 0, cooldown: 1, burn: 0, burnTime: 0, slow: 0, freeze: 0 };
}
beforeEach(() => { localStorage.clear(); });
afterEach(() => { wrapper?.unmount(); wrapper = undefined; vi.restoreAllMocks(); });

describe('轻量战斗画面和输入', () => {
  it('扩大后的射箭场地仍按原坐标计算拖动；松开只发射一次', async () => {
    wrapper = mount(ArcheryGame, { props: { paused: false } });
    const field = wrapper.get('.archery-surface');
    vi.spyOn(field.element, 'getBoundingClientRect').mockReturnValue({ left: 20, top: 10, width: 1280, height: 720 } as DOMRect);
    await pointer('pointerdown', 7, 600, 300);
    await pointer('pointerup', 7, 500, 400);
    await pointer('pointerup', 7, 500, 400);
    expect(read('archery').state.angle).toBeCloseTo(45);
    expect(read('archery').state.power).toBeCloseTo(180 + Math.hypot(50, 50) * 2.5);
    expect(read('archery').state.shots).toBe(1);
  });
  it('拉弓期间暂停或取消指针后，不会在恢复时意外放箭', async () => {
    wrapper = mount(ArcheryGame, { props: { paused: false } });
    await pointer('pointerdown', 2);
    await wrapper.setProps({ paused: true }); await wrapper.setProps({ paused: false });
    await pointer('pointerup', 2);
    await pointer('pointerdown', 3);
    await pointer('pointercancel', 3);
    await pointer('pointerup', 3);
    expect(read('archery')).toBeNull();
    expect(loop.paused()).toBe(true);
  });
  it('新箭种按钮扣除一次弹药，命中反馈不改变原来的物理和存档', async () => {
    store('archery', createArchery(1, 17));
    wrapper = mount(ArcheryGame, { props: { paused: false } });
    expect(loop.paused()).toBe(true);
    await button('重箭').trigger('click'); await button('瞄准辅助').trigger('click'); await button('放箭').trigger('click');
    const expected = read('archery').state;
    for (let i = 0; i < 180 && expected.phase === 'flight'; i++) { stepArchery(expected, .05); await advance(1); }
    expect(read('archery').state).toEqual(expected);
    expect(expected.enemyHP).toBeLessThan(expected.enemyMax);
    expect(expected.ammo.heavy).toBe(2);
    expect(wrapper.findAll('[data-battle-effect]').length).toBeGreaterThan(0);
    await wrapper.setProps({ paused: true });
    const feedback = wrapper.get('.archery-surface').html(); await advance(40);
    expect(wrapper.get('.archery-surface').html()).toBe(feedback);
    await wrapper.setProps({ paused: false }); await advance(200);
    expect(loop.paused()).toBe(true);
    expect(wrapper.findAll('[data-battle-effect]')).toHaveLength(0);
  });
  it('九类塔防敌人复用各自图形，塔炮转向与受击反馈正常', async () => {
    const s = createDefense(17); startDefense(s, createDefenseMeta()); s.spawnLeft = 0;
    const kinds: EnemyKind[] = ['normal','swarm','runner','armored','splitter','healer','shooter','shielded','boss'];
    s.enemies = kinds.map((kind, i) => ({ ...enemy(i + 1, kind), x: 380 + i })); s.nextId = 10;
    store('defense', s); wrapper = mount(DefenseGame, { props: { paused: false } });
    expect(wrapper.findAll('[data-enemy]')).toHaveLength(9);
    for (const kind of kinds) expect(wrapper.get(`[data-enemy="${kind}"] use`).attributes('href')).toBe(`#defense-${kind}`);
    await button('继续守卫').trigger('click'); await advance(12);
    expect(wrapper.get('[data-tower-barrel]').attributes('transform')).not.toContain('rotate(-25)');
    expect(wrapper.findAll('[data-effect-kind="hit"]').length).toBeGreaterThan(0);
    expect(validDefense(read('defense').state)).toBe(true);
    await wrapper.setProps({ paused: true }); const before = wrapper.get('.defense-surface').html(); await advance(50);
    expect(wrapper.get('.defense-surface').html()).toBe(before);
  });
  it('高攻速和群怪的短特效有上限，反馈不会被写进旧存档', async () => {
    const s = createDefense(89); startDefense(s, createDefenseMeta()); s.spawnLeft = 0;
    s.enemies = Array.from({ length: 65 }, (_, i) => enemy(i + 1, 'normal', 100000)); s.nextId = 66;
    s.cards = { multi: 6, chain: 6, blast: 6, speed: 6, overclock: 6, burn: 6 }; s.upgrades.rate = 30;
    store('defense', s); wrapper = mount(DefenseGame, { props: { paused: false } });
    await button('继续守卫').trigger('click'); await advance(100);
    expect(wrapper.findAll('[data-battle-effect]').length).toBeLessThanOrEqual(BATTLE_EFFECT_LIMIT);
    expect(read('defense').state).not.toHaveProperty('effects');
    expect(validDefense(read('defense').state)).toBe(true);
  });
  it('选卡时原地暂停，升级反馈可结束，选择后恢复同一波战斗', async () => {
    const s = createDefense(17); s.phase = 'choice';s.pendingUpgrades=1;s.choiceResume='playing'; s.offers = ['power','speed','multi'];
    store('defense', s); wrapper = mount(DefenseGame, { props: { paused: false } });
    expect(loop.paused()).toBe(true); expect(wrapper.findAll('.defense-card')).toHaveLength(3);
    await wrapper.get('.defense-upgrades button').trigger('click'); await advance(20);
    expect(loop.paused()).toBe(true); expect(read('defense').state.phase).toBe('choice');
    await wrapper.get('.defense-card').trigger('click');
    expect(read('defense').state.wave).toBe(1); expect(read('defense').state.cards.power).toBe(1);expect(read('defense').state.pendingUpgrades).toBe(0);
  });
  it('主动打开强化、免费刷新、暂存再打开与选卡消费均可实际点击', async () => {
    const s=createDefense(17);startDefense(s,createDefenseMeta());s.pendingUpgrades=4;
    store('defense',s);wrapper=mount(DefenseGame,{props:{paused:false}});
    await button('继续守卫').trigger('click');await wrapper.get('.defense-strengthen').trigger('click');
    expect(wrapper.findAll('.defense-card')).toHaveLength(3);expect(loop.paused()).toBe(true);
    const coins=read('defense').state.coins;
    for(let n=0;n<12;n++)await button('刷新三选一').trigger('click');
    expect(read('defense').state.coins).toBe(coins);expect(read('defense').state.pendingUpgrades).toBe(4);
    expect(wrapper.get('.defense-card span').text().length).toBeGreaterThan(25);
    const offers=read('defense').state.offers;await button('暂不选择').trigger('click');expect(read('defense').state.phase).toBe('playing');
    await wrapper.get('.defense-strengthen').trigger('click');expect(read('defense').state.offers).toEqual(offers);
    await wrapper.get('.defense-card').trigger('click');expect(read('defense').state.pendingUpgrades).toBe(3);
    expect(read('defense').state.phase).toBe('playing');expect(read('defense').state.wave).toBe(s.wave);
  });
});

describe('特效生命周期', () => {
  it('超过限额淘汰最旧特效，过期全部释放', () => {
    let effects: BattleEffect[] = [];
    for (let id = 0; id < 100; id++) addBattleEffect(effects, { id, kind: 'hit', x: 0, y: 0, color: '#fff', duration: .5, life: .5 });
    expect(effects).toHaveLength(BATTLE_EFFECT_LIMIT); expect(effects[0]!.id).toBe(100 - BATTLE_EFFECT_LIMIT);
    effects = stepBattleEffects(effects, .25); expect(effects[0]!.life).toBe(.25);
    expect(stepBattleEffects(effects, .3)).toHaveLength(0);
  });
});
