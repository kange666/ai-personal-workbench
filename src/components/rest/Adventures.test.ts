import { mount, type VueWrapper } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { nextTick } from 'vue';
import { gameSaveKey } from '../../composables/useGameSave';
import ArcheryGame from './ArcheryGame.vue';
import DefenseGame from './DefenseGame.vue';
import TowerAdventure from './TowerAdventure.vue';
import BreakoutAdventure from './BreakoutAdventure.vue';
import WiringAdventure from './WiringAdventure.vue';
import GomokuGame from './GomokuGame.vue';
import { createArchery, shootArchery, suggestArcheryAim, validArchery } from '../../utils/rest/archery';
import { createDefense, createDefenseMeta, startDefense, validDefense } from '../../utils/rest/defense';
import { createTowerAdventure } from '../../utils/rest/towerCampaign';
import { createBreakoutAdventure, startBreakoutAdventure } from '../../utils/rest/breakoutCampaign';
import { createCircuit } from '../../utils/rest/wiringCampaign';
import { createGomoku } from '../../utils/rest/gomoku';
import { createGomokuPuzzle } from '../../utils/rest/gomokuCampaign';
const loop=vi.hoisted(()=>({tick:(_dt:number)=>{},paused:():boolean=>true}));
vi.mock('../../composables/useGameLoop',()=>({useGameLoop:(paused:()=>boolean,tick:(dt:number)=>void)=>{loop.paused=paused;loop.tick=tick;}}));
let wrapper:VueWrapper|undefined;
const store=(id:string,state:unknown,best=0)=>localStorage.setItem(gameSaveKey(id),JSON.stringify({version:1,state,best}));
const read=(id:string)=>JSON.parse(localStorage.getItem(gameSaveKey(id))!);
async function advance(frames=30){for(let i=0;i<frames;i++)if(!loop.paused())loop.tick(.05);await nextTick();}
function button(label:string){return wrapper!.findAll('button').find(b=>b.text().includes(label))!;}
beforeEach(()=>{localStorage.clear();loop.tick=()=>{};loop.paused=()=>true;});
afterEach(()=>{wrapper?.unmount();wrapper=undefined;vi.restoreAllMocks();});

describe('新增游戏的实际组件交互',()=>{
  it('射箭辅助可使用，暂停不放箭，飞行中关闭后先等待继续',async()=>{
    wrapper=mount(ArcheryGame,{props:{paused:true}});await button('放箭').trigger('click');expect(read('archery')).toBeNull();
    await wrapper.setProps({paused:false});await button('瞄准辅助').trigger('click');const aimed=read('archery').state;expect(aimed.angle).toBeGreaterThan(0);
    await button('放箭').trigger('click');expect(read('archery').state.phase).toBe('flight');expect(read('archery').state.shots).toBe(1);
    await wrapper.setProps({paused:true});const before=JSON.stringify(read('archery').state);await advance(100);expect(JSON.stringify(read('archery').state)).toBe(before);
    wrapper.unmount();wrapper=mount(ArcheryGame,{props:{paused:false}});expect(wrapper.text()).toContain('继续对决');expect(loop.paused()).toBe(true);await button('继续对决').trigger('click');await advance(120);expect(validArchery(read('archery').state)).toBe(true);expect(read('archery').state.shots).toBe(1);
  });
  it('随机射箭胜利不会解锁章节，读档保留玩法',async()=>{
    const s=createArchery(18,91);s.mode='endless';s.enemyHP=1;const aim=suggestArcheryAim(s);shootArchery(s,aim.angle,aim.power);store('archery',s);
    wrapper=mount(ArcheryGame,{props:{paused:false}});await button('继续对决').trigger('click');await advance(150);expect(read('archery').state.phase).toBe('won');expect(read('archery-progress')).toBeNull();wrapper.unmount();expect(read('archery-progress').state.unlocked).toBe(0);
  });
  it('塔防启动、升级、选卡均落到存档；暂停和续存不会补算离开时间',async()=>{
    wrapper=mount(DefenseGame,{props:{paused:true}});await button('开始守卫').trigger('click');await advance();expect(wrapper.text()).toContain('开始守卫');
    await wrapper.setProps({paused:false});await button('开始守卫').trigger('click');await button('攻击').trigger('click');expect(read('defense').state.upgrades.attack).toBe(1);await advance(200);expect(validDefense(read('defense').state)).toBe(true);
    await wrapper.setProps({paused:true});const before=read('defense').state.time;await advance(100);wrapper.unmount();expect(read('defense').state.time).toBe(before);
    wrapper=mount(DefenseGame,{props:{paused:false}});expect(button('继续守卫').exists()).toBe(true);await advance(100);expect(read('defense').state.time).toBe(before);await button('继续守卫').trigger('click');await advance(400);expect(wrapper.findAll('.defense-card')).toHaveLength(3);
    const wave=read('defense').state.wave;await wrapper.get('.defense-card').trigger('click');expect(read('defense').state.wave).toBe(wave+1);expect(Object.keys(read('defense').state.cards)).toHaveLength(1);
  });
  it('塔防卡牌选择关闭重开保持同一组三选一，不会重新抽卡',async()=>{
    const s=createDefense(17);startDefense(s,createDefenseMeta());s.phase='choice';s.offers=['power','speed','multi'];store('defense',s,500);
    wrapper=mount(DefenseGame,{props:{paused:false}});expect(wrapper.findAll('.defense-card')).toHaveLength(3);await advance(100);wrapper.unmount();wrapper=mount(DefenseGame,{props:{paused:false}});expect(read('defense').state.offers).toEqual(['power','speed','multi']);expect(wrapper.text()).toContain('500');
  });
});

describe('扩展玩法、旧进度和章节隔离',()=>{
  it('五子棋残局只计算实际落子，奖励与最高分一致，重开保留结果',async()=>{
    wrapper=mount(GomokuGame,{props:{paused:false}});await wrapper.get('select').setValue('puzzle');
    expect(wrapper.get('.game-stats').text()).toContain('你的步数 0');
    const board=wrapper.get('.gomoku-board'),answer=createGomokuPuzzle(0).answer;
    vi.spyOn(board.element,'getBoundingClientRect').mockReturnValue({left:0,top:0,width:360,height:360} as DOMRect);
    await board.trigger('click',{clientX:19+answer%15*23,clientY:19+Math.floor(answer/15)*23});
    expect(wrapper.get('.game-stats').text()).toContain('你的步数 1');expect(wrapper.get('.game-stats').text()).toContain('本局得分 300');expect(read('gomoku').best).toBe(300);
    wrapper.unmount();wrapper=mount(GomokuGame,{props:{paused:false}});expect(wrapper.get('.game-stats').text()).toContain('本局得分 300');expect(wrapper.text()).toContain('解题成功');
  });
  it('叠塔响应空格，Vue 状态可复制回退，暂停禁止道具和落下',async()=>{
    const s=createTowerAdventure();s.run.phase='moving';s.run.movingX=150;store('tower-v2',s,85);
    wrapper=mount(TowerAdventure,{props:{paused:false},attachTo:document.body});await button('继续建造').trigger('click');expect(document.activeElement).toBe(wrapper.get('[data-game-focus]').element);await wrapper.get('[data-game-focus]').trigger('keydown',{code:'Space'});expect(read('tower-v2').state.run.blocks).toHaveLength(2);await button('回退一层').trigger('click');expect(read('tower-v2').state.run.blocks).toHaveLength(1);expect(read('tower-v2').state.undo).toBe(0);expect(document.activeElement).toBe(wrapper.get('[data-game-focus]').element);
    await wrapper.setProps({paused:true});await wrapper.get('[data-game-focus]').trigger('keydown',{code:'Space'});expect(read('tower-v2').state.run.blocks).toHaveLength(1);expect(wrapper.text()).toContain('85');
  });
  it('弹球重开续存、坐标换算和暂停保护，旧版本仍可进入',async()=>{
    const s=createBreakoutAdventure(0,'campaign',11);startBreakoutAdventure(s);store('breakout-v2',s,100);
    wrapper=mount(BreakoutAdventure,{props:{paused:false},attachTo:document.body});expect(loop.paused()).toBe(true);await button('继续游玩').trigger('click');expect(document.activeElement).toBe(wrapper.get('[data-game-focus]').element);await advance(25);const surface=wrapper.get('[data-game-focus]');vi.spyOn(surface.element,'getBoundingClientRect').mockReturnValue({left:20,width:480} as DOMRect);surface.element.dispatchEvent(new MouseEvent('pointermove',{clientX:350,bubbles:true}));await nextTick();await advance(25);expect(read('breakout-v2').state.paddleX).toBe(330);
    await wrapper.setProps({paused:true});const before=JSON.stringify(read('breakout-v2'));await surface.trigger('keydown',{key:'ArrowLeft'});await advance(40);expect(JSON.stringify(read('breakout-v2'))).toBe(before);
    await wrapper.setProps({paused:false});await wrapper.get('select').setValue('legacy');expect(wrapper.find('.breakout-game').exists()).toBe(true);
  });
  it('接线随机玩法恢复后不会当成章节，旧随机存档保留入口',async()=>{
    const s=createCircuit(45);s.mode='endless';store('wiring-v2',s);wrapper=mount(WiringAdventure,{props:{paused:false}});expect((wrapper.get('select').element as HTMLSelectElement).value).toBe('endless');await wrapper.get('select').setValue('legacy');expect(wrapper.find('.wiring-game').exists()).toBe(true);wrapper.unmount();expect(read('wiring-v2').state.mode).toBe('endless');
  });
  it('五子棋旧存档迁移后默认入门，执白时电脑先手；复盘期间不能落子',async()=>{
    store('gomoku',createGomoku(),90);wrapper=mount(GomokuGame,{props:{paused:false}});expect((wrapper.get('[aria-label="电脑难度"]').element as HTMLSelectElement).value).toBe('relaxed');await wrapper.findAll('select')[2].setValue('2');await advance(12);expect(read('gomoku').state.run.moves).toHaveLength(1);expect(read('gomoku').state.run.moves[0].player).toBe(1);await button('复盘').trigger('click');await wrapper.get('[data-game-focus]').trigger('keydown',{key:'Enter'});expect(read('gomoku').state.run.moves).toHaveLength(1);expect(read('gomoku').best).toBe(90);
  });
});
