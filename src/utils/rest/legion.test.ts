import { describe, expect, it } from 'vitest';
import { createLegion, LEGION_ACTOR_LIMIT, LEGION_EFFECT_LIMIT, legionStats, restartLegion, stepLegion, strengthenLegion, trainLegion, validLegion } from './legion';
import { LEGION_CARDS, LEGION_UNITS, type LegionStrategy } from './legionContent';
const run = (s: ReturnType<typeof createLegion>, seconds: number) => { for(let n=0;n<Math.round(seconds*10);n++)stepLegion(s,.1); };
describe('召唤军团规则、挂机与存档',()=>{
  it('六兵种与24种强化唯一，开局无需操作即战斗',()=>{const s=createLegion(7);expect(LEGION_UNITS).toHaveLength(6);expect(new Set(LEGION_CARDS.map(c=>c.id)).size).toBe(24);run(s,60);expect(s.kills).toBeGreaterThan(0);expect(s.wave).toBeGreaterThan(1);expect(validLegion(s)).toBe(true);});
  it('存档恢复后随机强化、移动和战斗结果完全一致',()=>{const a=createLegion(77);run(a,83);const b=JSON.parse(JSON.stringify(a));expect(validLegion(b)).toBe(true);run(a,90);run(b,90);expect(a).toEqual(b);});
  it('主动暂停和无效时间不推进战斗',()=>{const s=createLegion(1);s.paused=true;const copy=JSON.stringify(s);run(s,5);expect(JSON.stringify(s)).toBe(copy);s.paused=false;const active=JSON.stringify(s);for(const dt of [0,-1,NaN,Infinity,2])stepLegion(s,dt);expect(JSON.stringify(s)).toBe(active);});
  it('关闭自动强化后次数累积，手动连续消费后仍在同一局',()=>{const s=createLegion(2);s.options.strengthen=false;s.pending=3;const wave=s.wave;expect(strengthenLegion(s,'power')).toBe(true);expect(strengthenLegion(s,'haste')).toBe(true);expect(s.pending).toBe(1);expect(s.wave).toBe(wave);expect(legionStats(s,'sword').damage).toBeGreaterThan(16);run(s,1);expect(s.pending).toBe(1);});
  it('永久训练改变实际属性，重开保留训练金币和最高关',()=>{const s=createLegion(3);s.coins=1000;expect(trainLegion(s,'guard')).toBe(true);const hp=legionStats(s,'guard').hp;s.bestWave=12;s.cards={power:3};const fresh=restartLegion(s);expect(fresh.coins).toBe(s.coins);expect(fresh.training.guard).toBe(1);expect(fresh.bestWave).toBe(12);expect(fresh.cards).toEqual({});expect(fresh.allies.find(a=>a.kind==='guard')!.maxHP).toBe(hp);expect(validLegion(fresh)).toBe(true);});
  it('全军覆没自动整备，关闭续战则停留等待而非丢档',()=>{const s=createLegion(4);s.training.guard=2;s.coins=100;s.options.train=false;s.allies.forEach(a=>a.hp=0);stepLegion(s,.1);expect(s.phase).toBe('recover');expect(s.defeats).toBe(1);s.options.retry=false;run(s,10);expect(s.phase).toBe('recover');s.options.retry=true;run(s,5);expect(s.phase).toBe('battle');expect(s.allies.length).toBeGreaterThan(0);expect(s.training.guard).toBe(2);expect(validLegion(s)).toBe(true);});
  it('长时间僵持会结算回退，不能永久卡波；高层整备补齐已解锁军团',()=>{const s=createLegion(5);s.wave=60;s.bestWave=60;s.waveTime=120;stepLegion(s,.1);expect(s.phase).toBe('recover');expect(s.wave).toBe(44);run(s,4.1);expect(s.allies.length).toBeGreaterThanOrEqual(16);expect(s.allies.some(a=>a.kind==='priest')).toBe(true);expect(s.allies.some(a=>a.kind==='giant')).toBe(true);expect(validLegion(s)).toBe(true);});
  it('三类首领按章节轮换，技能确实产生援军或伤害',()=>{for(const [wave,kind] of [[10,'warlord'],[20,'behemoth'],[30,'necromancer']] as const){const s=createLegion(wave);s.wave=wave;s.bestWave=wave;s.phase='march';s.phaseTime=0;stepLegion(s,.1);expect(s.enemies.some(a=>a.kind===kind)).toBe(true);run(s,6);expect(s.log.some(line=>line.includes(kind==='behemoth'?'震地':'援军')||line.includes('战鼓'))).toBe(true);expect(validLegion(s)).toBe(true);}});
  it('存档拒绝重复ID、错阵营、无穷值和超量单位',()=>{const base=createLegion(6);for(const change of [(s:typeof base)=>{s.coins=Infinity;},(s:typeof base)=>{s.allies[0]!.side='enemy';},(s:typeof base)=>{s.allies[1]!.id=s.allies[0]!.id;},(s:typeof base)=>{s.effects=Array(100).fill({});},(s:typeof base)=>{s.options={} as typeof s.options;}]){const s=JSON.parse(JSON.stringify(base));change(s);expect(validLegion(s)).toBe(false);}});
  it('本关阵亡不补回，存档恢复也不复活；进入下一关才满血复活',()=>{
    const s=createLegion(9);
    s.allies=s.allies.filter(a=>a.kind!=='sword');s.essence=1000;s.options.replenish=true;
    run(s,2);expect(s.allies.some(a=>a.kind==='sword')).toBe(false);
    const saved:typeof s=JSON.parse(JSON.stringify(s));
    run(saved,1);expect(saved.allies.some(a=>a.kind==='sword')).toBe(false);
    s.phase='march';s.phaseTime=0;stepLegion(s,.1);
    const revived=s.allies.filter(a=>a.kind==='sword');
    expect(revived).toHaveLength(2);expect(revived.every(a=>a.hp===a.maxHP)).toBe(true);
    expect(s.formation.sword).toBe(2);expect(validLegion(s)).toBe(true);
  });
  it('新编队可扩充，但不能借切换策略重用阵亡名额',()=>{
    const s=createLegion(39);s.bestWave=12;s.strategy='arrows';s.essence=10000;
    s.allies=s.allies.filter(a=>a.kind!=='archer');s.options.train=false;
    run(s,1);
    // 保持同一关：敌人足够耐打且暂不攻击，避免自然过关触发合法复活。
    const enemy=s.enemies[0]!;enemy.hp=enemy.maxHP=1e12;enemy.cooldown=1000;s.spawnLeft=0;
    run(s,12);expect(s.formation.archer).toBe(5);
    expect(s.allies.filter(a=>a.kind==='archer')).toHaveLength(3);
    s.allies=s.allies.filter(a=>a.kind!=='archer');run(s,2);
    expect(s.allies.some(a=>a.kind==='archer')).toBe(false);
  });
  it('被燃烧击败的敌人不再攻击，传染击杀逐个结算一次',()=>{
    const s=createLegion(31);run(s,1);s.spawnLeft=0;s.allies.forEach(a=>a.cooldown=10);const e=s.enemies[0]!;e.x=s.allies[0]!.x+10;e.y=s.allies[0]!.y;e.hp=1;e.burn=10000;e.burnTime=1;e.cooldown=0;const hp=s.allies[0]!.hp;stepLegion(s,.1);expect(s.allies[0]!.hp).toBe(hp);
    const t=createLegion(32);run(t,1);t.spawnLeft=0;t.allies.forEach(a=>a.cooldown=10);const first=t.enemies[0]!;first.hp=0;first.burnTime=2;const second={...first,id:t.nextId++,hp:4,burnTime:0,x:first.x+10};t.enemies.push(second);t.cards={inferno:1};const before=t.kills;stepLegion(t,.1);expect(t.kills-before).toBe(2);expect(t.enemies).toHaveLength(0);stepLegion(t,.1);expect(t.kills-before).toBe(2);
  });
  it('四种策略各3个种子挂机15分钟，推进且有强化成长，无损坏存档或无限特效',()=>{
    const results=[];
    for(const strategy of ['balanced','arrows','elements','iron'] as LegionStrategy[])for(const seed of [101,207,811]){
      const s=createLegion(seed);s.strategy=strategy;
      for(let n=0;n<9000;n++){stepLegion(s,.1);if(n%1000===0){expect(validLegion(s),`${strategy}/${seed}/${n}`).toBe(true);expect(s.allies.length+s.enemies.length).toBeLessThanOrEqual(LEGION_ACTOR_LIMIT);expect(s.effects.length).toBeLessThanOrEqual(LEGION_EFFECT_LIMIT);}}
      results.push({strategy,seed,wave:s.bestWave,kills:s.kills,defeats:s.defeats,training:Math.max(...Object.values(s.training))});
      expect(s.bestWave,JSON.stringify(results.at(-1))).toBeGreaterThanOrEqual(10);expect(s.kills).toBeGreaterThan(70);expect(Math.max(...Object.values(s.training))).toBeGreaterThan(0);
    }
    console.info('军团挂机平衡',results);
  },30000);
});
