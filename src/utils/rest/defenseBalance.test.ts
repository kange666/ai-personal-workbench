// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { createDefense, createDefenseMeta, defenseFeatures, defenseStats, selectDefenseCore, startDefense, stepDefense, validDefense, type DefenseEnemy } from './defense';

function arena() {
  const s = createDefense(17, 'endless'); startDefense(s, createDefenseMeta());
  s.spawnLeft = 1; s.spawnTime = 1000;
  const enemy: DefenseEnemy = { id: 1, kind: 'normal', x: 380, y: 200, hp: 1e9, maxHP: 1e9, speed: 0, damage: 0, shield: 0, cooldown: 100, burn: 0, burnTime: 0, slow: 0, freeze: 0 };
  s.enemies = [enemy]; s.nextId = 2; return s;
}
const run = (s: ReturnType<typeof arena>, seconds: number) => { for (let n = 0; n < seconds * 20; n++) stepDefense(s, .05); };

describe('塔防弹幕、控制与堡垒平衡', () => {
  it('第12轮普攻足以击杀目标时，穿云弹幕仍先释放并计入进化伤害', () => {
    const s = arena(), f = defenseFeatures(s); f.evolutions.volley = 1; f.log.shots = 11;
    s.enemies[0]!.hp = 1; s.shotTime = 1 / defenseStats(s).rate;
    stepDefense(s, .01);
    expect(f.log.damage.evolution).toBe(1);
    expect(f.volleyBursts).toBe(1);
    expect(s.traces.filter(t => t.variant === 'volley').length).toBeGreaterThanOrEqual(5);
  });
  it('高频合并攻击跨过多个12轮时，所有弹幕按完整次数结算', () => {
    const s = arena(), f = defenseFeatures(s); f.evolutions.volley = 1; s.upgrades.rate = 1000;
    stepDefense(s, .2); const bursts = Math.floor(f.log.shots / 12);
    expect(bursts).toBeGreaterThan(1);
    expect(f.log.damage.evolution).toBeCloseTo(defenseStats(s).damage * 1.1 * bursts);
    expect(s.traces.length).toBeLessThanOrEqual(32);
  });
  it('极高冰冻等级与攻速也不能延长同一次冻结，敌人会定期恢复行动', () => {
    const s = arena(); s.upgrades.rate = 10000; s.cards = { slow: 10000, freeze: 10000 }; s.enemies[0]!.speed = 5;
    const x = defenseStats(s); expect(x.freezeDuration).toBeLessThanOrEqual(1.4); expect(x.freezeChance).toBeLessThanOrEqual(.45);
    const start = s.enemies[0]!.x; let free = 0, longest = 0, current = 0;
    for (let n = 0; n < 400; n++) { stepDefense(s, .05); if (s.enemies[0]!.freeze > 0) { current += .05; longest = Math.max(longest, current); } else { current = 0; free++; } }
    expect(free).toBeGreaterThan(150); expect(longest).toBeLessThanOrEqual(1.45);
    expect(s.enemies[0]!.x).toBeLessThan(start); expect(x.slow).toBeLessThan(.65);
  });
  it('首领冻结更短且解冻保护更长，选卡暂停不消耗保护时间', () => {
    const s = arena(); s.enemies[0]!.kind = 'boss'; s.upgrades.rate = 1000; s.cards = { slow: 100, freeze: 100 };
    run(s, .1); const e = s.enemies[0]!;
    expect(e.freeze).toBeGreaterThan(0); expect(e.freeze).toBeLessThan(.56); expect(e.freezeImmunity).toBeGreaterThan(4);
    const saved = JSON.stringify(s); s.phase = 'choice'; stepDefense(s, .1); s.phase = 'playing'; expect(JSON.stringify(s)).toBe(saved);
  });
  it('堡垒塔芯自带伤害护环，护环升级能以实际生命上限提高持续伤害', () => {
    const s = createDefense(17); selectDefenseCore(s, 'bastion'); startDefense(s, createDefenseMeta());
    const a = arena(); s.enemies = a.enemies; s.nextId = 2; s.spawnLeft = 1; s.spawnTime = 1000; s.shotTime = -100;
    run(s, 1); expect(defenseFeatures(s).log.damage.nova).toBeGreaterThan(0);
    const base = defenseStats(s).auraDPS; s.cards.aura = 2; s.upgrades.health = 10;
    expect(defenseStats(s).auraDPS).toBeGreaterThan(base * 2);
  });
  it('护环、脉冲与光束分别造成真实范围伤害并保持特效上限', () => {
    for (const id of ['aura', 'pulse', 'lance']) {
      const s = arena(); s.cards[id] = 2; s.enemies.push({ ...s.enemies[0]!, id: 2, x: 400 }); s.nextId = 3; s.shotTime = -100;
      run(s, 6); expect(defenseFeatures(s).log.damage.nova, id).toBeGreaterThan(0);
      expect(s.enemies.every(e => e.hp < e.maxHP), id).toBe(true);
      expect(s.traces.length).toBeLessThanOrEqual(32); expect(validDefense(JSON.parse(JSON.stringify(s)))).toBe(true);
    }
  });
});
