import { mount } from "@vue/test-utils";
import { afterEach, describe, expect, it } from "vitest";
import WiringGame from "../../components/rest/WiringGame.vue";
import { gameSaveKey } from "../../composables/useGameSave";
import { createWiringState, isWiringSolved, nextWiringRound, rotateWire, turnWire, validateWiringState, wiringAward, wiringLeaks, wiringPower, type WiringState } from "./wiring";

afterEach(() => { localStorage.clear(); });

function seededRandom(seed: number): () => number {
  return () => { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; return seed / 0x100000000; };
}
function solve(initial: WiringState): WiringState {
  let state = initial;
  for (let index = 0; index < state.tiles.length && !state.won; index++) {
    for (let rotation = 0; state.tiles[index] !== state.solution[index] && rotation < 4 && !state.won; rotation++) state = turnWire(state, index);
  }
  return state;
}

describe("休息站接线", () => {
  it("四次旋转恢复原状，方向按顺时针流转", () => {
    expect(rotateWire(1)).toBe(2);
    expect(rotateWire(8)).toBe(1);
    for (let mask = 1; mask < 16; mask++) expect(rotateWire(rotateWire(rotateWire(rotateWire(mask))))).toBe(mask);
  });
  it("不同种子和极端随机输入都生成完整可解且未通关的棋盘", () => {
    const puzzles = new Set<string>();
    for (let seed = 0; seed < 50; seed++) {
      const state = createWiringState(seededRandom(seed));
      expect(validateWiringState(state)).toBe(true);
      expect(isWiringSolved(state.solution)).toBe(true);
      expect(isWiringSolved(state.tiles)).toBe(false);
      const solved = solve(state);
      expect(solved.won).toBe(true);
      expect(validateWiringState(solved)).toBe(true);
      puzzles.add(state.solution.join(","));
    }
    expect(puzzles.size).toBeGreaterThan(35);
    expect(solve(createWiringState(() => 0)).won).toBe(true);
    expect(solve(createWiringState(() => 1)).won).toBe(true);
  });
  it("通电需要接口互相匹配，全部点亮但存在断口仍不算通关", () => {
    const state = createWiringState(seededRandom(10));
    expect(wiringPower(state.solution).size).toBe(25);
    expect(wiringLeaks(state.solution)).toBe(0);
    const tiles = [...state.solution];
    tiles[0] |= 1;
    expect(wiringPower(tiles).size).toBe(25);
    expect(wiringLeaks(tiles)).toBeGreaterThan(0);
    expect(isWiringSolved(tiles)).toBe(false);
    expect(wiringPower(Array(25).fill(1)).size).toBe(1);
  });
  it("只在首次通关计分，并在下一局保留累计分数", () => {
    const state = createWiringState(seededRandom(7));
    expect(nextWiringRound(state)).toBe(state);
    const solved = solve(state);
    expect(solved.score).toBe(wiringAward(solved.turns));
    expect(turnWire(solved, 0)).toBe(solved);
    const next = nextWiringRound(solved, seededRandom(8));
    expect(next.round).toBe(2);
    expect(next.score).toBe(solved.score);
    expect(next.turns).toBe(0);
    expect(next.won).toBe(false);
    expect(solve(next).score).toBeGreaterThan(solved.score);
    expect(wiringAward(1000)).toBe(100);
  });
  it("无效下标不修改棋盘，损坏存档不能作为有效状态恢复", () => {
    const state = createWiringState(seededRandom(4));
    for (const index of [-1, 25, 0.5, NaN]) expect(turnWire(state, index)).toBe(state);
    const invalid: unknown[] = [null, {}, { ...state, tiles: [1] }, { ...state, tiles: Array(25).fill(15) }, { ...state, solution: Array(25).fill(1) }, { ...state, score: -1 }, { ...state, turns: Infinity }, { ...state, won: true }, { ...state, lastAward: 100 }, { ...state, version: 2 }];
    for (const value of invalid) expect(validateWiringState(value)).toBe(false);
    expect(validateWiringState(JSON.parse(JSON.stringify(state)))).toBe(true);
  });
  it("组件暂停时不接受点击和键盘，关闭后恢复完整棋盘和旋转次数", async () => {
    const initial = createWiringState(seededRandom(11));
    localStorage.setItem(gameSaveKey("wiring"), JSON.stringify({ version: 1, state: initial, best: 0 }));
    const wrapper = mount(WiringGame, { props: { paused: true } });
    await wrapper.find(".wire-tile").trigger("click");
    await wrapper.find("[data-game-focus]").trigger("keydown", { key: "Enter" });
    expect(JSON.parse(localStorage.getItem(gameSaveKey("wiring"))!).state).toEqual(initial);
    await wrapper.setProps({ paused: false });
    await wrapper.find(".wire-tile").trigger("click");
    const saved = JSON.parse(localStorage.getItem(gameSaveKey("wiring"))!).state;
    expect(saved.turns).toBe(1);
    expect(saved.tiles[0]).toBe(rotateWire(initial.tiles[0]));
    wrapper.unmount();
    const reopened = mount(WiringGame, { props: { paused: false } });
    expect(reopened.text()).toContain("旋转 1 次");
    reopened.unmount();
    expect(JSON.parse(localStorage.getItem(gameSaveKey("wiring"))!).state).toEqual(saved);
  });
  it("组件从损坏存档安全开新局并保留已有最高分", () => {
    localStorage.setItem(gameSaveKey("wiring"), JSON.stringify({ version: 1, state: { tiles: null }, best: 900 }));
    const wrapper = mount(WiringGame, { props: { paused: false } });
    expect(wrapper.findAll(".wire-tile")).toHaveLength(25);
    expect(wrapper.text()).toContain("900");
    wrapper.unmount();
    expect(validateWiringState(JSON.parse(localStorage.getItem(gameSaveKey("wiring"))!).state)).toBe(true);
  });
});
