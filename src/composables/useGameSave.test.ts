import { mount } from "@vue/test-utils";
import { defineComponent } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { gameSaveKey, restStorageWarning, useGameSave } from "./useGameSave";

type State = { moves: number; board: number[] };
const create = (): State => ({ moves: 0, board: [1, 2] });
const validate = (value: unknown): value is State => !!value && typeof value === "object" && Number.isInteger((value as State).moves) && Array.isArray((value as State).board);
function session(id = "test") {
  let game!: ReturnType<typeof useGameSave<State>>;
  const wrapper = mount(defineComponent({ setup() { game = useGameSave(id, create, validate); return () => null; } }));
  return { game, wrapper };
}
beforeEach(() => { localStorage.clear(); restStorageWarning.value = ""; });
afterEach(() => vi.restoreAllMocks());
describe("休息站本机存档", () => {
  it("关闭后恢复完整进度、最高分，重开仅重置进度", () => {
    let s = session(); s.game.state.value.moves = 8; s.game.state.value.board = [3, 1]; s.game.recordScore(120); s.wrapper.unmount();
    s = session(); expect(s.game.state.value).toEqual({ moves: 8, board: [3, 1] }); expect(s.game.best.value).toBe(120);
    s.game.recordScore(20); s.game.restart(); expect(s.game.state.value).toEqual(create()); expect(s.game.best.value).toBe(120); s.wrapper.unmount();
  });
  it("六款存档互不覆盖", () => {
    const a = session("a"), b = session("b"); a.game.state.value.moves = 9; a.game.recordScore(50); b.game.recordScore(12); a.wrapper.unmount(); b.wrapper.unmount();
    expect(JSON.parse(localStorage.getItem(gameSaveKey("a"))!).state.moves).toBe(9);
    expect(JSON.parse(localStorage.getItem(gameSaveKey("b"))!).best).toBe(12);
  });
  it.each(["{", "null", "[]", JSON.stringify({ version: 2, state: create(), best: 85 }), JSON.stringify({ version: 1, state: { moves: "bad" }, best: 85 })])("损坏/未知版本存档安全恢复：%s", raw => {
    localStorage.setItem(gameSaveKey("test"), raw); const s = session(); expect(s.game.state.value).toEqual(create()); expect(restStorageWarning.value).toBeTruthy(); s.wrapper.unmount();
  });
  it("拒绝非法分数并保留其他窗口的更高纪录", () => {
    const s = session(); s.game.recordScore(NaN); s.game.recordScore(-1); expect(s.game.best.value).toBe(0);
    localStorage.setItem(gameSaveKey("test"), JSON.stringify({ version: 1, state: create(), best: 90 })); s.game.recordScore(25); expect(s.game.best.value).toBe(90); s.wrapper.unmount();
  });
  it("存储容量不足不打断游戏，并明确说明未保存", () => {
    const s = session(); vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new DOMException("quota"); });
    s.game.state.value.moves++; expect(() => s.game.save()).not.toThrow(); expect(s.game.storageError.value).toBe(true); expect(restStorageWarning.value).toContain("无法保存"); s.wrapper.unmount();
  });
  it("切走窗口立即保存，销毁后解绑监听", () => {
    const s = session(); s.game.state.value.moves = 12; window.dispatchEvent(new Event("blur")); expect(JSON.parse(localStorage.getItem(gameSaveKey("test"))!).state.moves).toBe(12);
    s.wrapper.unmount(); const setter = vi.spyOn(Storage.prototype, "setItem"); window.dispatchEvent(new Event("pagehide")); expect(setter).not.toHaveBeenCalled();
  });
  it("较长的合法存档可恢复，超过读写上限时保留上一份记录", () => {
    let s = session(); s.game.state.value.board = Array(150_000).fill(1); s.game.recordScore(77); s.wrapper.unmount();
    s = session(); expect(s.game.state.value.board).toHaveLength(150_000); expect(s.game.best.value).toBe(77);
    const previous = localStorage.getItem(gameSaveKey("test")); s.game.state.value.board = Array(600_000).fill(1); s.game.save();
    expect(s.game.storageError.value).toBe(true); expect(localStorage.getItem(gameSaveKey("test"))).toBe(previous); s.wrapper.unmount();
  });
});
