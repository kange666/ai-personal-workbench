import { describe, expect, it } from "vitest";
import { actFishing, advanceFishing, createFishing, fishPoints, isFishingState, reelPointer, reelTarget, type FishingState } from "./fishing";

function waitForBite(state: FishingState) {
  for (let i = 0; i < 100 && state.phase === "waiting"; i += 1) advanceFishing(state, 0.05);
}

function catchFish(state: FishingState) {
  actFishing(state, () => 0.5);
  waitForBite(state);
  expect(state.phase).toBe("bite");
  actFishing(state);
  for (let pull = 0; pull < 3; pull += 1) {
    const target = reelTarget(state);
    for (let frame = 0; frame < 180; frame += 1) {
      const position = reelPointer(state);
      if (position >= target.start + 0.03 && position <= target.start + target.width - 0.03 && state.reelElapsed - state.lastPull >= 0.2) break;
      advanceFishing(state, 0.02);
    }
    actFishing(state);
  }
}

describe("休闲钓鱼", () => {
  it("完成抛竿、等待、提竿、三次收线并按鱼重结算，继续垂钓保留累计", () => {
    const state = createFishing();
    catchFish(state);
    expect(state.phase).toBe("caught");
    expect(state.caught).toBe(1);
    expect(state.score).toBe(fishPoints(state.fish!));
    expect(state.collection[1]).toBe(1);
    const score = state.score;
    catchFish(state);
    expect(state.caught).toBe(2);
    expect(state.score).toBe(score * 2);
    expect(isFishingState(JSON.parse(JSON.stringify(state)))).toBe(true);
  });

  it("等待阶段重复按键不会提前钓到鱼或重置倒计时", () => {
    const state = createFishing();
    actFishing(state, () => 0.3);
    advanceFishing(state, 0.5);
    expect(actFishing(state)).toBe(false);
    expect(state.elapsed).toBe(0.5);
    expect(state.score).toBe(0);
  });

  it("错过咬钩或收线超时会逃脱，并允许重新抛竿", () => {
    const state = createFishing();
    actFishing(state);
    waitForBite(state);
    for (let i = 0; i < 50; i += 1) advanceFishing(state, 0.05);
    expect(state.phase).toBe("escaped");
    expect(state.escapeReason).toBe("late");
    expect(state.score).toBe(0);
    actFishing(state);
    waitForBite(state);
    actFishing(state);
    for (let i = 0; i < 181; i += 1) advanceFishing(state, 0.05);
    expect(state.phase).toBe("escaped");
    expect(isFishingState(state)).toBe(true);
  });

  it("在绿色区间之外收线失败，连按不会重复计分", () => {
    const state = createFishing();
    actFishing(state);
    waitForBite(state);
    actFishing(state);
    actFishing(state);
    expect(state.phase).toBe("escaped");
    expect(state.escapeReason).toBe("miss");
    catchFish(state);
    const score = state.score;
    actFishing(state);
    actFishing(state);
    expect(state.phase).toBe("waiting");
    expect(state.score).toBe(score);
  });

  it("恢复中的浮漂、鱼种、计时与收线进度可无损续玩", () => {
    const state = createFishing();
    actFishing(state, () => 0.98);
    waitForBite(state);
    actFishing(state);
    advanceFishing(state, 0.4);
    actFishing(state);
    expect(state.reelHits).toBe(1);
    const restored = JSON.parse(JSON.stringify(state)) as FishingState;
    expect(isFishingState(restored)).toBe(true);
    advanceFishing(restored, 0.04);
    advanceFishing(state, 0.04);
    expect(restored).toEqual(state);
  });

  it("拦截损坏存档、未知鱼种、非法分数和已过期的进行中阶段", () => {
    expect(isFishingState(createFishing())).toBe(true);
    const state = createFishing();
    actFishing(state, () => 0.5);
    for (const invalid of [null, {}, { ...state, score: Infinity }, { ...state, score: 99 }, { ...state, fish: { species: 99, weight: 1 } }, { ...state, collection: [0] }, { ...state, caught: 2 }, { ...state, elapsed: 5 }, { ...state, reelHits: -1 }, { ...state, lastPull: 4 }, { ...state, phase: "reeling", reelElapsed: 9 }, { ...state, escapeReason: "miss" }]) expect(isFishingState(invalid)).toBe(false);
  });

  it("无效时间不会改变状态，完成后计时停止", () => {
    const state = createFishing();
    catchFish(state);
    const before = structuredClone(state);
    for (const dt of [0, -1, NaN, Infinity, 0.1]) advanceFishing(state, dt);
    expect(state).toEqual(before);
  });
});
