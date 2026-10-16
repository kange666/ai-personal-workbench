import { mount } from '@vue/test-utils';
import { expect, it } from 'vitest';
import { h } from 'vue';
import { createLegion, stepLegion, type LegionEffect } from '../../utils/rest/legion';
import LegionBattlefield from './LegionBattlefield.vue';
import LegionEffects from './LegionEffects.vue';

it('旧存档无来源字段的弹道可绘制，生命周期推进后弹体确实移动',async()=>{
  const effect:LegionEffect={id:1,kind:'arrow',x:100,y:200,tx:400,ty:220,life:.32,text:''};
  const wrapper=mount(LegionEffects,{props:{effects:[effect],cards:{}}});
  const start=wrapper.get('[data-legion-projectile]').attributes('transform');
  await wrapper.setProps({effects:[{...effect,life:.16}]});
  expect(wrapper.get('[data-legion-projectile]').attributes('transform')).not.toBe(start);
  await wrapper.setProps({effects:[{...effect,x:400,y:220,life:0}]});
  expect(wrapper.html()).not.toMatch(/NaN|Infinity/);wrapper.unmount();
});

it('同一页多战场的绘制资源互不冲突，重载静止画面不会自行推进',()=>{
  const state=createLegion(17);state.paused=true;
  const pair=mount({render:()=>h('div',[h(LegionBattlefield,{state}),h(LegionBattlefield,{state})])});
  const [a,b]=pair.findAllComponents(LegionBattlefield);
  const idA=a!.get('linearGradient').attributes('id'),idB=b!.get('linearGradient').attributes('id');
  expect(idA).not.toBe(idB);expect(a!.html()).toContain(`url(#${idA})`);
  const frozen=a!.html();stepLegion(state,.1);expect(a!.html()).toBe(frozen);
  pair.unmount();
});

it('阵亡人数显示为当前存活/入场编队，重新渲染不会产生复活',async()=>{
  const state=createLegion(19);state.allies=state.allies.filter(a=>a.kind!=='sword');
  const wrapper=mount(LegionBattlefield,{props:{state}});
  expect(wrapper.text()).toContain('军团 4/6 人');
  await wrapper.setProps({state:{...state,elapsed:state.elapsed+1}});
  expect(wrapper.findAll('[data-legion-unit="sword"]')).toHaveLength(0);
  expect(state.formation.sword).toBe(2);wrapper.unmount();
});
