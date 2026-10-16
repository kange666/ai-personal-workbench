// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { closeDefenseOffers, createDefense, createDefenseMeta, defenseOffers, enterDefenseEndless, migrateDefense, pickDefenseCard, startDefense, stepDefense, validDefense, DEFENSE_CARDS } from './defense';
import { defenseCardDetails } from './defenseCardDetails';

describe('可累计的强化奖励', () => {
  it('连续三波不选卡也自动出怪，奖励逐次累加，保存后保留', () => {
    const s=createDefense(19);startDefense(s,createDefenseMeta());
    for(let n=1;n<=3;n++){
      s.spawnLeft=0;s.enemies=[];stepDefense(s,.1);
      expect(s.wave).toBe(n+1);expect(s.phase).toBe('playing');expect(s.pendingUpgrades).toBe(n);
      stepDefense(s,.2);expect(s.enemies.length).toBeGreaterThan(0);
    }
    expect(validDefense(JSON.parse(JSON.stringify(s)))).toBe(true);
  });
  it('100次刷新免费且不限制次数，关闭选卡保留次数与当前抽牌，选一次只扣一次', () => {
    const s=createDefense(5);startDefense(s,createDefenseMeta());s.pendingUpgrades=3;
    for(let n=0;n<50;n++)stepDefense(s,.1);
    const wave=s.wave,enemies=structuredClone(s.enemies),time=s.time,coins=s.coins;
    expect(defenseOffers(s)).toBe(true);
    for(let n=0;n<100;n++){
      const before=[...s.offers];expect(defenseOffers(s,true)).toBe(true);
      expect(s.offers).toHaveLength(3);expect(new Set(s.offers).size).toBe(3);expect(s.offers).not.toEqual(before);
      expect(s.pendingUpgrades).toBe(3);expect(s.coins).toBe(coins);stepDefense(s,.1);expect(s.time).toBe(time);
    }
    const offers=[...s.offers],seed=s.seed;expect(closeDefenseOffers(s)).toBe(true);expect(s.pendingUpgrades).toBe(3);
    expect(defenseOffers(s)).toBe(true);expect(s.offers).toEqual(offers);expect(s.seed).toBe(seed);
    expect(pickDefenseCard(s,offers[0]!)).toBe(true);expect(s.phase).toBe('choice');expect(s.pendingUpgrades).toBe(2);
    expect(s.wave).toBe(wave);expect(s.enemies).toEqual(enemies);expect(s.time).toBe(time);expect(validDefense(s)).toBe(true);
    expect(pickDefenseCard(s,offers[0]!)).toBe(false);expect(s.pendingUpgrades).toBe(2);
  });
  it('第30波也发奖励，选卡后回到胜利界面，转无尽保留累计次数', () => {
    const s=createDefense(7);startDefense(s,createDefenseMeta());s.wave=30;s.spawnLeft=0;s.enemies=[];
    stepDefense(s,.1);expect(s.phase).toBe('won');expect(s.pendingUpgrades).toBe(1);
    defenseOffers(s);expect(s.choiceResume).toBe('won');closeDefenseOffers(s);expect(s.phase).toBe('won');
    expect(enterDefenseEndless(s)).toBe(true);expect(s.phase).toBe('playing');expect(s.wave).toBe(31);expect(s.pendingUpgrades).toBe(1);
  });
  it('旧版波末选卡原地迁移，保留原牌、金币、升级和最高波数口径', () => {
    const current=createDefense(17);current.phase='choice';current.wave=9;current.offers=['power','speed','health'];current.coins=123;current.cards={power:2};
    const legacy:Record<string,unknown>={...current,version:1};delete legacy.pendingUpgrades;delete legacy.choiceResume;
    const before=JSON.stringify(legacy),next=migrateDefense(legacy)!;
    expect(JSON.stringify(legacy)).toBe(before);expect(next.phase).toBe('playing');expect(next.wave).toBe(10);expect(next.pendingUpgrades).toBe(1);
    expect(next.offers).toEqual(current.offers);expect(next.cards).toEqual(current.cards);expect(next.coins).toBe(123);expect(validDefense(next)).toBe(true);
    expect(migrateDefense({...legacy,coins:NaN})).toBeNull();expect(validDefense({...next,pendingUpgrades:-1})).toBe(false);
  });
  it('36张卡均有数值、触发条件或上限的详细说明', () => {
    const s=createDefense(1);
    for(const card of DEFENSE_CARDS){const text=defenseCardDetails(s,card.id).description;expect(text.length,card.id).toBeGreaterThan(25);expect(text,card.id).toMatch(/\d/);}
  });
});
