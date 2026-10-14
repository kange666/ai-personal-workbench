import { mount } from "@vue/test-utils";
import { afterEach, describe, expect, it } from "vitest";
import SokobanGame from "../../components/rest/SokobanGame.vue";
import { gameSaveKey } from "../../composables/useGameSave";
import { createSokobanState, getSokobanBoard, isSokobanSolved, moveSokoban, moveSokobanSnapshot, nextSokobanLevel, restartSokobanLevel, SOKOBAN_DIRECTIONS, SOKOBAN_LEVELS, sokobanAward, sokobanDestination, undoSokoban, validateSokobanState, type SokobanBoard, type SokobanDirection, type SokobanState } from "./sokoban";

afterEach(() => { localStorage.clear(); });

const directions = Object.keys(SOKOBAN_DIRECTIONS) as SokobanDirection[];
const opposite: Record<SokobanDirection, SokobanDirection> = { up: "down", right: "left", down: "up", left: "right" };

function reachable(board: SokobanBoard, player: number, boxes: readonly number[]): Map<number, SokobanDirection[]> {
  const paths = new Map<number, SokobanDirection[]>([[player, []]]);
  const queue = [player];
  for (let cursor = 0; cursor < queue.length; cursor++) {
    for (const direction of directions) {
      const next = sokobanDestination(board, queue[cursor], direction);
      if (next === null || boxes.includes(next) || paths.has(next)) continue;
      paths.set(next, [...paths.get(queue[cursor])!, direction]);
      queue.push(next);
    }
  }
  return paths;
}

// 老关卡使用独立搜索，新关卡回放反向生成的证书，交叉验证实际移动规则。
function solveLevel(level: number): SokobanDirection[] | null {
  const board = getSokobanBoard(level);
  const queue = [{ player: board.player, boxes: board.boxes, path: [] as SokobanDirection[] }];
  const seen = new Set<string>();
  for (let cursor = 0; cursor < queue.length && cursor < 100000; cursor++) {
    const current = queue[cursor];
    if (isSokobanSolved(board, current.boxes)) return current.path;
    const walks = reachable(board, current.player, current.boxes);
    const key = `${Math.min(...walks.keys())}|${[...current.boxes].sort((left, right) => left - right).join(",")}`;
    if (seen.has(key)) continue;
    seen.add(key);
    for (const box of current.boxes) for (const direction of directions) {
      const stand = sokobanDestination(board, box, opposite[direction]);
      const target = sokobanDestination(board, box, direction);
      if (stand === null || target === null || !walks.has(stand) || current.boxes.includes(target)) continue;
      if (!board.goals.includes(target)) {
        const blocked = directions.map(candidate => sokobanDestination(board, target, candidate) === null);
        if ((blocked[0] || blocked[2]) && (blocked[1] || blocked[3])) continue;
      }
      queue.push({ player: box, boxes: current.boxes.map(position => position === box ? target : position), path: [...current.path, ...walks.get(stand)!, direction] });
    }
  }
  return null;
}

function solveState(state: SokobanState): SokobanState {
  const letters: Record<string,SokobanDirection> = { U:'up', R:'right', D:'down', L:'left' };
  const solution = state.level < 6 ? solveLevel(state.level) : [...SOKOBAN_LEVELS[state.level].solution].map(c => letters[c]);
  expect(solution, `第 ${state.level + 1} 关应该可解`).not.toBeNull();
  for (const direction of solution!) {
    state = moveSokoban(state, direction);
    expect(validateSokobanState(state)).toBe(true);
  }
  return state;
}

describe("休息站推箱子", () => {
  it("八十关均有实际可执行的通关路线，逐关完成且不重复加分", () => {
    let state = createSokobanState();
    let previousScore = 0;
    SOKOBAN_LEVELS.forEach((level, index) => {
      expect(level.map.every(row => row.length === level.map[0].length)).toBe(true);
      const board = getSokobanBoard(index);
      expect(board.boxes.length).toBe(board.goals.length);
      expect(state.level).toBe(index);
      state = solveState(state);
      expect(state.completed).toBe(true);
      expect(state.score).toBe(previousScore + sokobanAward(state.moves, state.pushes));
      expect(moveSokoban(state, "left")).toBe(state);
      previousScore = state.score;
      const advanced = nextSokobanLevel(state);
      if (index === SOKOBAN_LEVELS.length - 1) expect(advanced).toBe(state);
      else {
        expect(advanced.moves).toBe(0);
        expect(advanced.history).toHaveLength(0);
        expect(validateSokobanState(advanced)).toBe(true);
        state = advanced;
      }
    });
    expect(state.awards.every(award => award >= 100)).toBe(true);
  }, 60000);
  it("撞墙、箱后有墙和连续箱子都不移动、不增加步数", () => {
    const state = createSokobanState();
    expect(moveSokoban(state, "right")).toBe(state);
    const board = getSokobanBoard(0);
    expect(moveSokobanSnapshot(board, { player: 16, boxes: [15], moves: 0, pushes: 0 }, "left")).toBeNull();
    expect(moveSokobanSnapshot(board, { player: 19, boxes: [18, 17], moves: 0, pushes: 0 }, "left")).toBeNull();
    expect(sokobanDestination({ width: 3, height: 3, walls: [], goals: [], player: 0, boxes: [] }, 2, "right")).toBeNull();
    expect(nextSokobanLevel(state)).toBe(state);
  });
  it("只对实际推动计数，撤销可完整恢复位置、步数和已结算得分", () => {
    const state = createSokobanState();
    const walked = moveSokoban(state, "up");
    expect(walked.moves).toBe(1);
    expect(walked.pushes).toBe(0);
    expect(undoSokoban(walked)).toEqual(state);
    const pushed = moveSokoban(state, "left");
    expect(pushed.pushes).toBe(1);
    expect(pushed.moves).toBe(1);
    const won = moveSokoban(pushed, "left");
    expect(won.completed).toBe(true);
    expect(won.score).toBe(970);
    expect(undoSokoban(won)).toEqual(pushed);
    expect(moveSokoban(undoSokoban(won), "left").score).toBe(970);
    expect(undoSokoban(state)).toBe(state);
  });
  it("重开仅清除本关，下一关保留已完成关卡分数", () => {
    const first = solveState(createSokobanState());
    const next = nextSokobanLevel(first);
    const second = solveState(next);
    const retry = restartSokobanLevel(second);
    expect(retry.level).toBe(1);
    expect(retry.score).toBe(first.score);
    expect(retry.awards[1]).toBe(0);
    expect(retry.moves).toBe(0);
    expect(retry.history).toEqual([]);
    expect(validateSokobanState(retry)).toBe(true);
    expect(sokobanAward(1000, 1000)).toBe(100);
  });
  it("限制撤销记录大小，长局存档仍可验证和恢复", () => {
    let state = createSokobanState();
    for (let index = 0; index < 230; index++) state = moveSokoban(state, index % 2 === 0 ? "up" : "down");
    expect(state.moves).toBe(230);
    expect(state.history).toHaveLength(200);
    expect(validateSokobanState(state)).toBe(true);
    for (let index = 0; index < 200; index++) state = undoSokoban(state);
    expect(state.moves).toBe(30);
    expect(state.history).toHaveLength(0);
    expect(validateSokobanState(state)).toBe(true);
    expect(undoSokoban(state)).toBe(state);
  });
  it("拒绝越界、墙内、重叠、虚假通关、伪造得分和损坏撤销记录", () => {
    const state = createSokobanState();
    const moved = moveSokoban(state, "up");
    const invalid: unknown[] = [null, {}, { ...state, version: 2 }, { ...state, level: 80 }, { ...state, player: 0 }, { ...state, player: 1000 }, { ...state, boxes: [state.player] }, { ...state, boxes: [1, 1] }, { ...state, completed: true }, { ...state, awards: [1000, 0, 0, 0, 0, 0] }, { ...state, score: 1000 }, { ...state, moves: -1 }, { ...state, pushes: 1 }, { ...moved, history: [null] }, { ...moved, history: [{ ...moved.history[0], player: 8 }] }];
    for (const value of invalid) expect(validateSokobanState(value)).toBe(false);
    expect(validateSokobanState(JSON.parse(JSON.stringify(moved)))).toBe(true);
  });
  it("组件保存棋盘、撤销和最高分，暂停阻止键盘与方向按钮", async () => {
    const initial = createSokobanState();
    localStorage.setItem(gameSaveKey("sokoban"), JSON.stringify({ version: 1, state: initial, best: 0 }));
    const wrapper = mount(SokobanGame, { props: { paused: true } });
    await wrapper.find("[data-game-focus]").trigger("keydown", { key: "ArrowLeft" });
    await wrapper.find('[aria-label="向左移动"]').trigger("click");
    expect(JSON.parse(localStorage.getItem(gameSaveKey("sokoban"))!).state).toEqual(initial);
    await wrapper.setProps({ paused: false });
    await wrapper.find("[data-game-focus]").trigger("keydown", { key: "ArrowLeft" });
    await wrapper.find('[aria-label="向左移动"]').trigger("click");
    wrapper.unmount();
    const won = JSON.parse(localStorage.getItem(gameSaveKey("sokoban"))!);
    expect(won.state.completed).toBe(true);
    expect(won.best).toBe(970);
    const reopened = mount(SokobanGame, { props: { paused: false } });
    await reopened.find("[data-game-focus]").trigger("keydown", { key: "z" });
    const undone = JSON.parse(localStorage.getItem(gameSaveKey("sokoban"))!);
    expect(undone.state.moves).toBe(1);
    expect(undone.state.history).toHaveLength(1);
    expect(undone.state.score).toBe(0);
    expect(undone.best).toBe(970);
    reopened.unmount();
  });
  it("组件拒绝损坏棋盘并保留已有最高分", () => {
    localStorage.setItem(gameSaveKey("sokoban"), JSON.stringify({ version: 1, state: { level: 80 }, best: 970 }));
    const wrapper = mount(SokobanGame, { props: { paused: false } });
    expect(wrapper.findAll(".sokoban-player")).toHaveLength(1);
    expect(wrapper.text()).toContain("970");
    wrapper.unmount();
    expect(validateSokobanState(JSON.parse(localStorage.getItem(gameSaveKey("sokoban"))!).state)).toBe(true);
  });
});
