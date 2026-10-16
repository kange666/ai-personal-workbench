import { mount, type VueWrapper } from '@vue/test-utils';
import { nextTick } from 'vue';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import DefenseGame from './DefenseGame.vue';
import DefenseTower from './DefenseTower.vue';
import DefenseEffects from './DefenseEffects.vue';
import { defenseBarrelLength, defenseProjectileOrigin } from '../../utils/rest/defenseContent';
import { gameSaveKey } from '../../composables/useGameSave';
import { createDefense, createDefenseMeta, defenseStats, startDefense, type DefenseState, type EnemyKind } from '../../utils/rest/defense';
import { BATTLE_EFFECT_LIMIT, type BattleEffect } from '../../utils/rest/battleEffects';
const loop=vi.hoisted(()=>({paused:():boolean=>true,tick:(_dt:number)=>{}}));
vi.mock('../../composables/useGameLoop',()=>({useGameLoop:(paused:()=>boolean,tick:(dt:number)=>void)=>{loop.paused=paused;loop.tick=tick;}}));
let wrapper:VueWrapper|undefined;
const store=(state:DefenseState,best=0)=>localStorage.setItem(gameSaveKey('defense'),JSON.stringify({version:1,state,best}));
const read=()=>JSON.parse(localStorage.getItem(gameSaveKey('defense'))!);
const button=(label:string)=>wrapper!.findAll('button').find(b=>b.text().includes(label))!;
async function advance(frames:number){for(let i=0;i<frames;i++)if(!loop.paused())loop.tick(.05);await nextTick();}
beforeEach(()=>{localStorage.clear();loop.paused=()=>true;loop.tick=()=>{};});
afterEach(()=>{wrapper?.unmount();wrapper=undefined;vi.restoreAllMocks();});

describe('塔防扩展的真实组件交互',()=>{
  it('所有塔芯近身弹道均朝目标前进，远处弹道仍从对应等级炮口发出',()=>{
    for(const core of ['rapid','cannon','tesla','bastion','classic'] as const)for(const tier of [1,3,5])for(const angle of [0,.7,2,4,5]){
      for(const distance of [5,24,40,180]){
        const target={x:300+Math.cos(angle)*distance,y:170+Math.sin(angle)*distance},origin=defenseProjectileOrigin(core,tier,target);
        const remaining=Math.hypot(target.x-origin.x,target.y-origin.y);
        expect(remaining).toBeGreaterThan(0);expect(remaining).toBeLessThan(distance);
        expect((target.x-origin.x)*(target.x-300)+(target.y-origin.y)*(target.y-170)).toBeGreaterThan(0);
        if(distance===180)expect(Math.hypot(origin.x-300,origin.y-170)).toBeCloseTo(defenseBarrelLength(core,tier));
      }
    }
    expect(defenseProjectileOrigin('cannon',5,{x:300,y:170})).toEqual({x:300,y:170,angle:0});
  });
  it('塔芯轮廓、等级结构与组合进化的外观相互独立，不同等级可清楚辨别',async()=>{
    wrapper=mount(DefenseTower,{props:{core:'rapid',tier:1,recoil:.8,hurt:0,direction:20,shield:30,maxShield:45,evolved:false,paused:false,skills:{fire:0,ice:0,chain:0,blast:0}}});
    expect(wrapper.find('[data-tower-silhouette="gatling"]').exists()).toBe(true);expect(wrapper.findAll('[data-tower-structure]')).toHaveLength(0);
    await wrapper.setProps({tier:5,evolved:true,evolutions:{firestorm:1,froststorm:1,volley:1,bulwark:1},skills:{fire:5,ice:5,chain:5,blast:5}});
    expect(wrapper.findAll('[data-tower-structure]')).toHaveLength(4);expect(wrapper.findAll('[data-tower-evolution]')).toHaveLength(3);
    expect(wrapper.get('[data-tower-shield]').attributes('stroke-dasharray')).toBeUndefined();
    for(const [core,shape] of [['cannon','howitzer'],['tesla','reactor'],['bastion','fortress']] as const){await wrapper.setProps({core});expect(wrapper.find('[data-tower-silhouette="'+shape+'"]').exists()).toBe(true);}
    expect(wrapper.find('filter').exists()).toBe(false);
  });
  it('六种弹体、混合元素和四种范围技有独立表现，延迟命中在等待期不可见',()=>{
    const shots=['kinetic','shell','fire','ice','plasma','volley'] as const;
    const effects:BattleEffect[]=shots.map((variant,id)=>({id,variant,kind:'shot',x:400,y:200,fromX:340,fromY:170,color:'#ffe0a0',duration:.2,life:.1}));
    for(const [id,variant] of (['fire','ice','shield','nova'] as const).entries())effects.push({id:id+6,variant,kind:'burst',x:400,y:200,color:'#ffe0a0',radius:90,duration:.5,life:.5,delay:.1});
    wrapper=mount(DefenseEffects,{props:{effects,tier:5,skills:{fire:5,ice:5,chain:5,blast:5}}});
    for(const variant of shots)expect(wrapper.find('[data-projectile-body="'+variant+'"]').exists()).toBe(true);
    expect(wrapper.find('[data-projectile-layer="frost"]').exists()).toBe(true);
    expect(wrapper.findAll('[data-effect-kind="burst"]').every(n=>n.attributes('opacity')==='0')).toBe(true);
    expect(wrapper.find('filter').exists()).toBe(false);
  });
  it('四种塔芯切换改变塔炮，等级与技能改变可见模块，旧版场景结构保留',async()=>{
    const s=createDefense(1);s.cards={power:35,speed:35,burn:3,slow:3,chain:3,blast:3};s.upgrades.health=40;
    store(s);wrapper=mount(DefenseGame,{props:{paused:false}});
    for(const [name,id] of [['重炮塔芯','cannon'],['电磁塔芯','tesla'],['堡垒塔芯','bastion'],['机关塔芯','rapid']]){
      await button(name!).trigger('click');expect(wrapper.get('[data-central-tower]').attributes('data-core')).toBe(id);
    }
    expect(wrapper.get('[data-central-tower]').attributes('data-tower-tier')).toBe('5');
    expect(wrapper.findAll('[data-tower-module]')).toHaveLength(4);
    expect(wrapper.get('.defense-surface').attributes('viewBox')).toBe('-100 -35 800 450');
    await button('开始守卫').trigger('click');expect(read().state.features.core).toBe('rapid');
  });
  it('搜索全技能可以连续选择，已解锁条件正确，次数耗尽才关闭面板',async()=>{
    const s=createDefense(9);s.phase='choice';s.pendingUpgrades=3;s.choiceResume='playing';s.offers=['power','speed','health'];
    store(s);wrapper=mount(DefenseGame,{props:{paused:false}});
    await wrapper.get('[aria-label="搜索全部强化"]').setValue('碎冰');
    expect(wrapper.get('.defense-card').attributes('disabled')).toBeDefined();expect(wrapper.get('.defense-card').text()).toContain('需要');
    await wrapper.get('[aria-label="搜索全部强化"]').setValue('吸血');expect(wrapper.findAll('.defense-card')).toHaveLength(1);
    for(let n=1;n<=3;n++){
      await wrapper.get('.defense-card').trigger('click');expect(read().state.cards.leech).toBe(n);expect(read().state.pendingUpgrades).toBe(3-n);
      if(n<3){expect(read().state.phase).toBe('choice');expect(wrapper.get('[aria-label="搜索全部强化"]').element).toHaveProperty('value','吸血');expect(loop.paused()).toBe(true);}
    }
    expect(wrapper.find('.defense-choice-overlay').exists()).toBe(false);expect(read().state.phase).toBe('playing');
  });
  it('批量购买和自动升级可操作、可保存，预留金币始终有效',async()=>{
    const s=createDefense(91);s.coins=1200;store(s);wrapper=mount(DefenseGame,{props:{paused:false}});
    await wrapper.get('[aria-label="批量升级数量"]').setValue('5');await wrapper.get('.defense-upgrades button').trigger('click');
    expect(read().state.upgrades.attack).toBe(5);await button('开始守卫').trigger('click');
    await wrapper.get('[aria-label="自动升级预留金币"]').setValue(200);await wrapper.get('[aria-label="自动升级优先属性"]').setValue('attack');
    await wrapper.get('[aria-label="自动购买基础升级"]').setValue(true);await advance(40);
    expect(read().state.upgrades.attack).toBeGreaterThan(5);expect(read().state.coins).toBeGreaterThanOrEqual(200);
    wrapper.unmount();wrapper=mount(DefenseGame,{props:{paused:false}});expect(read().state.features.auto).toMatchObject({enabled:true,priority:'attack',batch:5,reserve:200});
    expect(loop.paused()).toBe(true);
  });
  it('金币不足时显示下一等级所需金币，不显示免费升级',()=>{
    const s=createDefense(5);s.coins=0;store(s);wrapper=mount(DefenseGame,{props:{paused:false}});
    const upgrade=wrapper.get('.defense-upgrades button');expect(upgrade.text()).toContain('需 12 ◈');
    expect(upgrade.text()).not.toContain('+0级');expect(upgrade.attributes('disabled')).toBeDefined();
  });
  it('属性详情、限制规则和有效伤害统计可展开，显示实际无上限属性',async()=>{
    const s=createDefense(19);s.upgrades.rate=200;s.upgrades.range=200;store(s);wrapper=mount(DefenseGame,{props:{paused:false}});
    const details=wrapper.get('details.defense-build');(details.element as HTMLDetailsElement).open=true;await details.trigger('toggle');await nextTick();
    expect(details.text()).toContain('暴击伤害');expect(details.text()).toContain('每秒护盾回复');expect(details.text()).toContain('本局实际战斗统计');expect(details.get('summary').text()).toContain('收起');
    expect(details.text()).toContain('6目标和8连锁硬上限');expect(details.text()).toContain('1,750');expect(details.text()).toContain(defenseStats(s).rate.toLocaleString('zh-CN',{maximumFractionDigits:2}));
  });
  it('旧局转入后波数、金币、现场和最高分保留，首次选择新塔芯不清档',async()=>{
    const s=createDefense(9,'endless');startDefense(s,createDefenseMeta());s.wave=47;s.coins=1234;s.hp=123;s.cards={power:8,range:12};s.pendingUpgrades=5;delete s.features;
    store(s,9999);wrapper=mount(DefenseGame,{props:{paused:false}});expect(wrapper.get('[data-central-tower]').attributes('data-core')).toBe('classic');
    await button('继续守卫').trigger('click');await button('电磁塔芯').trigger('click');
    expect(read().state).toMatchObject({wave:47,coins:1234,hp:123,mode:'endless',pendingUpgrades:5,cards:s.cards});expect(read().best).toBe(9999);expect(read().state.enemies).toEqual(s.enemies);
  });
  it('新增怪物有独立图形，升级后的弹道和爆破特效受绘制预算约束',async()=>{
    const s=createDefense(13,'endless');startDefense(s,createDefenseMeta());s.spawnLeft=1;s.spawnTime=100;s.upgrades.rate=100;
    const kinds:EnemyKind[]=['charger','warder','hexer','brood','leech'];
    s.enemies=kinds.map((kind,i)=>({id:i+1,kind,x:365+i*5,y:200,hp:1e8,maxHP:1e8,speed:0,damage:0,shield:0,cooldown:1,burn:0,burnTime:0,slow:0,freeze:0}));s.nextId=6;s.cards={burn:5,blast:5,multi:20,chain:20};
    store(s);wrapper=mount(DefenseGame,{props:{paused:false}});for(const kind of kinds)expect(wrapper.get(`[data-enemy="${kind}"] use`).attributes('href')).toBe('#defense-'+kind);
    await button('继续守卫').trigger('click');await advance(6);
    expect(wrapper.findAll('[data-effect-kind="shot"]').length).toBeGreaterThan(0);expect(wrapper.findAll('[data-battle-effect]').length).toBeLessThanOrEqual(BATTLE_EFFECT_LIMIT);
    expect(wrapper.find('[data-effect-variant="fire"]').exists()).toBe(true);expect(read().state).not.toHaveProperty('effects');
  });
});
