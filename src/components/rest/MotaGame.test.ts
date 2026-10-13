import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import MotaGame from "./MotaGame.vue";
import type { MotaPlayer } from "../../services/motaPlayer";

const api = vi.hoisted(() => ({ getMotaPlayer: vi.fn(), hideMotaPlayer: vi.fn(), positionMotaPlayer: vi.fn(), discardMotaPlayer: vi.fn() }));
vi.mock("../../services/motaPlayer", () => api);
let wrapper: VueWrapper | null;
let player: MotaPlayer;
const suspend = vi.fn(), resume = vi.fn();
beforeEach(() => {
  vi.clearAllMocks();
  player = Object.assign(document.createElement("div"), { ruffle: () => ({ suspend, resume, load: vi.fn() }) }) as MotaPlayer;
  api.getMotaPlayer.mockResolvedValue(player);
});
afterEach(() => { wrapper?.unmount(); wrapper = null; });

it("暂停时隐藏并停住原版引擎，恢复时继续，关闭后不销毁游戏节点", async () => {
  wrapper = mount(MotaGame, { props: { paused: false } });
  await flushPromises();
  expect(resume).toHaveBeenCalledOnce();
  expect(api.positionMotaPlayer).toHaveBeenCalled();
  await wrapper.setProps({ paused: true });
  expect(suspend).toHaveBeenCalledOnce();
  expect(api.hideMotaPlayer).toHaveBeenCalledOnce();
  await wrapper.setProps({ paused: false });
  expect(resume).toHaveBeenCalledTimes(2);
  wrapper.unmount(); wrapper = null;
  expect(suspend).toHaveBeenCalledTimes(2);
  expect(api.discardMotaPlayer).not.toHaveBeenCalled();
});

it("加载尚未完成就关闭，不显示或启动遗留的播放器", async () => {
  let finish!: (player: MotaPlayer) => void;
  api.getMotaPlayer.mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  wrapper = mount(MotaGame, { props: { paused: false } });
  wrapper.unmount(); wrapper = null;
  finish(player); await flushPromises();
  expect(resume).not.toHaveBeenCalled();
  expect(api.positionMotaPlayer).not.toHaveBeenCalled();
});

it("加载期间收到暂停状态，完成后仍不开始运行", async () => {
  let finish!: (player: MotaPlayer) => void;
  api.getMotaPlayer.mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  wrapper = mount(MotaGame, { props: { paused: false } });
  await wrapper.setProps({ paused: true }); finish(player); await flushPromises();
  expect(suspend).toHaveBeenCalledOnce();
  expect(resume).not.toHaveBeenCalled();
  expect(api.positionMotaPlayer).not.toHaveBeenCalled();
});

it("弹窗滚动后更新可见边界，卸载后不再移动持久播放器", async () => {
  const panel = document.createElement("section"); panel.className = "rest-dialog";
  document.body.appendChild(panel);
  const bounds = new DOMRect(0, 0, 600, 300);
  vi.spyOn(panel, "getBoundingClientRect").mockReturnValue(bounds);
  try {
    wrapper = mount(MotaGame, { props: { paused: false }, attachTo: panel });
    await flushPromises();
    const surface = wrapper.get(".mota-surface").element;
    const rect = new DOMRect(10, -100, 570, 410);
    vi.spyOn(surface, "getBoundingClientRect").mockReturnValue(rect);
    api.positionMotaPlayer.mockClear();
    panel.dispatchEvent(new Event("scroll"));
    expect(api.positionMotaPlayer).toHaveBeenCalledWith(rect, bounds);
    wrapper.unmount(); wrapper = null;
    api.positionMotaPlayer.mockClear();
    panel.dispatchEvent(new Event("scroll"));
    expect(api.positionMotaPlayer).not.toHaveBeenCalled();
  } finally { panel.remove(); }
});

it("显示加载错误并可重试，重新开始只清除魔塔运行实例", async () => {
  api.getMotaPlayer.mockRejectedValueOnce(new Error("读取失败")).mockResolvedValue(player);
  wrapper = mount(MotaGame, { props: { paused: false } }); await flushPromises();
  expect(wrapper.get('[role="alert"]').text()).toContain("加载失败");
  await wrapper.get("button").trigger("click"); await flushPromises();
  expect(wrapper.find('[role="alert"]').exists()).toBe(false);
  (wrapper.vm as unknown as { restart(): void }).restart(); await flushPromises();
  expect(api.discardMotaPlayer).toHaveBeenCalledOnce();
  expect(api.getMotaPlayer).toHaveBeenCalledTimes(3);
});
