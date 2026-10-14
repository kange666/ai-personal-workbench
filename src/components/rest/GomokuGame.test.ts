import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mount, type VueWrapper } from "@vue/test-utils";
import { nextTick } from "vue";
import GomokuGame from "./GomokuGame.vue";
import { gameSaveKey } from "../../composables/useGameSave";
import { isGomokuState } from "../../utils/rest/gomoku";

const loop = vi.hoisted(() => ({ tick: (_dt: number) => {}, paused: (): boolean => false }));
vi.mock("../../composables/useGameLoop", () => ({ useGameLoop: (paused: () => boolean, tick: (dt: number) => void) => { loop.tick = tick; loop.paused = paused; } }));
let wrapper: VueWrapper | undefined;
const stored = () => JSON.parse(localStorage.getItem(gameSaveKey("gomoku"))!);
async function advance() { for (let i = 0; i < 10; i += 1) if (!loop.paused()) loop.tick(0.05); await nextTick(); }

beforeEach(() => localStorage.clear());
afterEach(() => { wrapper?.unmount(); wrapper = undefined; vi.restoreAllMocks(); });

describe("五子棋界面与电脑回合", () => {
  it("方向键和空格落子，电脑仅回应一次，悔棋回到玩家回合", async () => {
    wrapper = mount(GomokuGame, { props: { paused: false } });
    const surface = wrapper.get("[data-game-focus]");
    await surface.trigger("keydown", { key: "ArrowRight" });
    await surface.trigger("keydown", { key: " " });
    expect(stored().state.run.moves).toEqual([{ index: 113, player: 1 }]);
    expect(wrapper.text()).toContain("电脑正在思考");
    await advance();
    await advance();
    expect(stored().state.run.moves).toHaveLength(2);
    expect(wrapper.text()).toContain("轮到你了");
    expect(isGomokuState(stored().state.run)).toBe(true);
    await wrapper.get("button").trigger("click");
    expect(stored().state.run.moves).toHaveLength(0);
    await advance();
    expect(stored().state.run.moves).toHaveLength(0);
  });

  it("关闭于电脑回合，恢复后继续思考且不会重复落子", async () => {
    wrapper = mount(GomokuGame, { props: { paused: false } });
    await wrapper.get("[data-game-focus]").trigger("keydown", { key: "Enter" });
    wrapper.unmount();
    wrapper = mount(GomokuGame, { props: { paused: true } });
    await advance();
    expect(stored().state.run.moves).toHaveLength(1);
    await wrapper.setProps({ paused: false });
    await advance();
    await advance();
    expect(stored().state.run.moves).toHaveLength(2);
  });

  it("暂停阻止落子和悔棋，长按空格不落子", async () => {
    wrapper = mount(GomokuGame, { props: { paused: true } });
    await wrapper.get("[data-game-focus]").trigger("keydown", { key: " " });
    expect(wrapper.text()).toContain("你的步数 0");
    await wrapper.setProps({ paused: false });
    await wrapper.get("[data-game-focus]").trigger("keydown", { key: " ", repeat: true });
    expect(wrapper.text()).toContain("你的步数 0");
    await wrapper.get("[data-game-focus]").trigger("keydown", { key: " " });
    await wrapper.setProps({ paused: true });
    expect(wrapper.get("button").attributes("disabled")).toBeDefined();
  });

  it("鼠标点击按缩放后的棋盘坐标落子", async () => {
    wrapper = mount(GomokuGame, { props: { paused: false } });
    const board = wrapper.get("svg");
    vi.spyOn(board.element, "getBoundingClientRect").mockReturnValue({ x: 20, y: 40, left: 20, top: 40, right: 380, bottom: 400, width: 360, height: 360, toJSON: () => ({}) });
    await board.trigger("click", { clientX: 20 + 19 + 23 * 3, clientY: 40 + 19 + 23 * 5 });
    expect(stored().state.run.moves[0]).toEqual({ index: 78, player: 1 });
  });
});
