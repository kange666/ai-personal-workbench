import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mount, type VueWrapper } from "@vue/test-utils";
import { nextTick } from "vue";
import FishingGame from "./FishingGame.vue";
import { gameSaveKey } from "../../composables/useGameSave";
import { isFishingState } from "../../utils/rest/fishing";

const loop = vi.hoisted(() => ({ tick: (_dt: number) => {}, paused: (): boolean => false }));
vi.mock("../../composables/useGameLoop", () => ({ useGameLoop: (paused: () => boolean, tick: (dt: number) => void) => { loop.tick = tick; loop.paused = paused; } }));
let wrapper: VueWrapper | undefined;
const stored = () => JSON.parse(localStorage.getItem(gameSaveKey("fishing"))!);
async function advance(seconds: number) { for (let i = 0; i < Math.round(seconds / 0.02); i += 1) if (!loop.paused()) loop.tick(0.02); await nextTick(); }

beforeEach(() => { localStorage.clear(); vi.spyOn(Math, "random").mockReturnValue(0.5); });
afterEach(() => { wrapper?.unmount(); wrapper = undefined; vi.restoreAllMocks(); });

describe("钓鱼界面与持久化", () => {
  it("键盘完整钓获并记录最高分，重新挂载保持鱼获", async () => {
    wrapper = mount(FishingGame, { props: { paused: false }, attachTo: document.body });
    const surface = wrapper.get("[data-game-focus]");
    await surface.trigger("keydown", { key: " " });
    expect(wrapper.text()).toContain("等待咬钩");
    expect(document.activeElement).toBe(surface.element);
    await advance(3.1);
    expect(wrapper.text()).toContain("立即提竿");
    await surface.trigger("keydown", { key: "Enter" });
    await advance(0.4);
    await surface.trigger("keydown", { key: " " });
    await advance(0.6);
    await surface.trigger("keydown", { key: " " });
    await advance(1.34);
    await surface.trigger("keydown", { key: " " });
    expect(wrapper.text()).toContain("继续垂钓");
    expect(stored().state.caught).toBe(1);
    expect(stored().best).toBeGreaterThan(0);
    expect(isFishingState(stored().state)).toBe(true);
    wrapper.unmount();
    wrapper = mount(FishingGame, { props: { paused: false } });
    expect(wrapper.text()).toContain("1 尾");
    expect(wrapper.text()).toContain("青尾鲈");
  });

  it("暂停期间按键无效，关闭后保存精确倒计时并从原处继续", async () => {
    wrapper = mount(FishingGame, { props: { paused: false } });
    await wrapper.get("button").trigger("click");
    await advance(1.2);
    await wrapper.setProps({ paused: true });
    await wrapper.get("[data-game-focus]").trigger("keydown", { key: " " });
    await advance(4);
    wrapper.unmount();
    const elapsed = stored().state.elapsed;
    expect(elapsed).toBeCloseTo(1.2);
    wrapper = mount(FishingGame, { props: { paused: false } });
    expect(wrapper.text()).toContain("准备好了，继续");
    await advance(5);
    expect(wrapper.text()).toContain("准备好了，继续");
    await wrapper.get("button").trigger("click");
    await advance(1.9);
    expect(wrapper.text()).toContain("立即提竿");
  });

  it("在咬钩即将超时时关闭，重开等玩家确认后才继续计时", async () => {
    wrapper = mount(FishingGame, { props: { paused: false } });
    await wrapper.get("button").trigger("click");
    await advance(5.1);
    expect(wrapper.text()).toContain("立即提竿");
    wrapper.unmount();
    const savedElapsed = stored().state.elapsed;
    wrapper = mount(FishingGame, { props: { paused: false } });
    await advance(10);
    expect(wrapper.text()).toContain("计时已暂停");
    expect(stored().state.elapsed).toBe(savedElapsed);
    await wrapper.get("[data-game-focus]").trigger("keydown", { key: " " });
    await wrapper.get("[data-game-focus]").trigger("keydown", { key: " " });
    expect(wrapper.text()).toContain("收线三次");
  });

  it("拦截长按重复事件，重新开始只清本次进度", async () => {
    wrapper = mount(FishingGame, { props: { paused: false } });
    await wrapper.get("[data-game-focus]").trigger("keydown", { key: " ", repeat: true });
    expect(wrapper.text()).toContain("湖面很静");
    await wrapper.get("button").trigger("click");
    (wrapper.vm as unknown as { restart: () => void }).restart();
    await nextTick();
    expect(wrapper.text()).toContain("湖面很静");
    expect(stored().state.phase).toBe("idle");
  });
});
