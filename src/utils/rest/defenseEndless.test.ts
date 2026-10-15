import { describe, expect, it } from 'vitest';
import { createDefense, createDefenseMeta, DEFENSE_CARDS, DEFENSE_UPGRADES, defenseOffers, defenseStats, defenseUpgradeCost, enterDefenseEndless, pickDefenseCard, settleDefense, startDefense, startDefenseWave, stepDefense, upgradeDefense, validDefense, validDefenseMeta } from './defense';

describe('无尽守卫', () => {
  it('可以从第一波进入无尽，第三十波结束仍提供强化并继续出怪', () => {
    const s = createDefense(17, 'endless'); startDefense(s, createDefenseMeta());
    expect(s.mode).toBe('endless'); expect(s.wave).toBe(1);
    s.wave = 30; s.spawnLeft = 0; s.enemies = [];
    stepDefense(s, .1);
    expect(s.phase).toBe('choice'); expect(s.offers).toHaveLength(3);
    expect(pickDefenseCard(s, s.offers[0]!)).toBe(true);
    expect(s.wave).toBe(31); expect(s.phase).toBe('playing');
    stepDefense(s, .2); expect(s.enemies.length).toBeGreaterThan(0);
    expect(validDefense(s)).toBe(true);
    const campaign = createDefense(17); startDefense(campaign, createDefenseMeta());
    campaign.wave = 30; campaign.spawnLeft = 0; stepDefense(campaign, .1);
    expect(campaign.phase).toBe('won');
  });

  it('已有战斗或选卡转无尽只切换规则，不重置当前进度与随机数', () => {
    for (const phase of ['playing', 'choice'] as const) {
      const s = createDefense(91); startDefense(s, createDefenseMeta());
      for (let i = 0; i < 80; i++) stepDefense(s, .1);
      s.phase = phase; if (phase === 'choice') s.offers = ['power', 'blast', 'speed'];
      const before = structuredClone(s);
      expect(enterDefenseEndless(s)).toBe(true);
      expect(s).toEqual({ ...before, mode: 'endless' });
      expect(enterDefenseEndless(s)).toBe(false); expect(validDefense(s)).toBe(true);
    }
    const won = createDefense(1); won.phase = 'won'; won.wave = 30; won.coins = 300;
    expect(enterDefenseEndless(won)).toBe(true); expect(won.wave).toBe(31);
    expect(won.coins).toBe(300); expect(won.phase).toBe('playing');
    won.phase = 'lost'; expect(enterDefenseEndless(won)).toBe(false);
  });

  it('六类基础升级均可超过100级，攻速和射程达到画面上限后仍能增伤', () => {
    const s = createDefense(7, 'endless'); s.coins = 1e8;
    for (const u of DEFENSE_UPGRADES) s.upgrades[u.id] = 100;
    const before = defenseStats(s);
    for (const u of DEFENSE_UPGRADES) {
      const damage = defenseStats(s).damage;
      expect(upgradeDefense(s, u.id)).toBe(true); expect(s.upgrades[u.id]).toBe(101);
      if (u.id === 'rate' || u.id === 'range') expect(defenseStats(s).damage).toBeGreaterThan(damage);
    }
    const after = defenseStats(s);
    expect(after.rate).toBe(18); expect(after.range).toBe(300);
    expect(after.maxHP).toBeGreaterThan(before.maxHP);
    expect(after.regen).toBeGreaterThan(before.regen); expect(after.coinBoost).toBeGreaterThan(before.coinBoost);
    expect(validDefense(JSON.parse(JSON.stringify(s)))).toBe(true);
    s.mode = 'campaign'; expect(upgradeDefense(s, 'attack')).toBe(false);
  });

  it('无尽后期价格持续增长但不再指数失控，前20级与闯关相同', () => {
    const s = createDefense(1, 'endless'), campaign = createDefense(1);
    for (let level = 0; level <= 20; level++) {
      s.upgrades.attack = campaign.upgrades.attack = level;
      expect(defenseUpgradeCost(s, 'attack')).toBe(defenseUpgradeCost(campaign, 'attack'));
    }
    s.upgrades.attack = 100; const cost = defenseUpgradeCost(s, 'attack');
    s.upgrades.attack++; expect(defenseUpgradeCost(s, 'attack')).toBeGreaterThan(cost);
    expect(cost).toBeLessThan(100000);
    s.upgrades.attack = Number.MAX_SAFE_INTEGER - 1;
    expect(Number.isSafeInteger(defenseUpgradeCost(s, 'attack'))).toBe(true);
  });

  it('所有强化牌6级后仍可选择，每次精研继续增伤且存档有效', () => {
    const s = createDefense(5, 'endless'); s.wave = 50;
    s.cards = Object.fromEntries(DEFENSE_CARDS.map(c => [c.id, 6]));
    const before = defenseStats(s).damage;
    defenseOffers(s);
    expect(s.phase).toBe('choice'); expect(s.wave).toBe(50); expect(s.offers).toHaveLength(3);
    const id = s.offers[0]!; expect(pickDefenseCard(s, id)).toBe(true);
    expect(s.cards[id]).toBe(7); expect(defenseStats(s).damage).toBeGreaterThan(before);
    expect(validDefense(s)).toBe(true);
    for (let i = 0; i < 80; i++) { defenseOffers(s); expect(pickDefenseCard(s, s.offers[0]!)).toBe(true); }
    expect(validDefense(JSON.parse(JSON.stringify(s)))).toBe(true);
  });

  it('超过旧波数和金币上限仍可出怪、保存、复现战斗及记录最高波数', () => {
    const s = createDefense(73, 'endless'); s.wave = 100001; s.coins = 2e9; s.hp = 2000;
    s.upgrades.health = 100; s.upgrades.attack = 120; s.cards = { power: 8, burn: 9 };
    startDefenseWave(s);
    for (let i = 0; i < 50; i++) stepDefense(s, .1);
    expect(s.enemies.length).toBeGreaterThan(0); expect(validDefense(s)).toBe(true);
    const restored = JSON.parse(JSON.stringify(s));
    for (let i = 0; i < 20; i++) { stepDefense(s, .1); stepDefense(restored, .1); }
    expect(restored).toEqual(s); expect(validDefense(restored)).toBe(true);
    const meta = createDefenseMeta(); s.phase = 'choice'; settleDefense(s, meta);
    expect(meta.bestWave).toBe(100001); expect(validDefenseMeta(meta)).toBe(true);
    expect(validDefense({ ...s, wave: Infinity })).toBe(false);
    expect(validDefense({ ...s, upgrades: { ...s.upgrades, attack: 1.5 } })).toBe(false);
  });

  it('高波数怪物继续变强，不沿用闯关的血量封顶', () => {
    const hp = (wave: number) => { const s = createDefense(1, 'endless'); s.wave = wave; startDefenseWave(s); stepDefense(s, .2); expect(validDefense(s)).toBe(true); return s.enemies[0]!.maxHP; };
    expect(hp(200000)).toBeGreaterThan(hp(100000));
  });

  it('同屏满65只时保留待入场队列，有空位才继续出怪', () => {
    const s = createDefense(4, 'endless'); startDefense(s, createDefenseMeta());
    s.enemies = Array.from({ length: 65 }, (_, i) => ({ id: i + 1, kind: 'normal' as const, x: 560, y: 200, hp: 1e6, maxHP: 1e6, speed: 0, damage: 1, shield: 0, cooldown: 1, burn: 0, burnTime: 0, slow: 0, freeze: 0 }));
    s.nextId = 66; s.spawnLeft = 2; s.spawnTime = 0;
    for (let i = 0; i < 50; i++) stepDefense(s, .1);
    expect(s.enemies).toHaveLength(65); expect(s.spawnLeft).toBe(2); expect(validDefense(s)).toBe(true);
    s.enemies.shift(); stepDefense(s, .1);
    expect(s.enemies).toHaveLength(65); expect(s.spawnLeft).toBe(1); expect(validDefense(s)).toBe(true);
  });
});
