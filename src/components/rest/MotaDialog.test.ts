import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { nextTick } from "vue";
import { afterEach, expect, it, vi } from "vitest";
import RestRailCard from "./RestRailCard.vue";
import { discardMotaPlayer, type MotaPlayer } from "../../services/motaPlayer";

let wrapper: VueWrapper | null = null;
const originalRuntime = Object.getOwnPropertyDescriptor(window, "RufflePlayer");
afterEach(() => {
  wrapper?.unmount(); wrapper = null;
  discardMotaPlayer();
  document.body.innerHTML = "";
  document.querySelectorAll('script[src*="mota21"]').forEach(script => script.remove());
  if (originalRuntime) Object.defineProperty(window, "RufflePlayer", originalRuntime);
  else Reflect.deleteProperty(window, "RufflePlayer");
});

// 使用实际右栏、弹窗、魔塔组件和播放器服务；只模拟 Ruffle，不能据此证明原版游玩通过。
it("从右栏打开魔塔后，暂停、关闭续开和确认重置共同保持正确的实例与焦点", async () => {
  const players: MotaPlayer[] = [];
  const load = vi.fn(async () => {}), suspend = vi.fn(), resume = vi.fn();
  const createPlayer = vi.fn(() => {
    const player = Object.assign(document.createElement("div"), { ruffle: () => ({ load, suspend, resume }) }) as MotaPlayer;
    players.push(player); return player;
  });
  Object.defineProperty(window, "RufflePlayer", { configurable: true, value: { newest: () => ({ createPlayer }) } });
  document.body.innerHTML = '<div id="app"></div>';
  wrapper = mount(RestRailCard, { attachTo: document.getElementById("app")! });
  const launcher = wrapper.get<HTMLButtonElement>('[aria-label="打开魔塔"]');
  launcher.element.focus();
  await launcher.trigger("click");
  await vi.waitFor(async () => {
    await flushPromises();
    expect(document.querySelector('script[src*="mota21"]')).not.toBeNull();
  }, { timeout: 4000, interval: 20 });
  document.querySelector('script[src*="mota21"]')!.dispatchEvent(new Event("load"));
  await flushPromises();
  const surface = document.querySelector<HTMLElement>(".mota-surface")!;
  // jsdom 没有排版能力，匿名矩形提供可见画幅，不涉及访问预览站点。
  vi.spyOn(surface, "getBoundingClientRect").mockReturnValue(new DOMRect(40, 100, 570, 410));
  vi.spyOn(document.querySelector<HTMLElement>(".rest-dialog")!, "getBoundingClientRect").mockReturnValue(new DOMRect(20, 20, 760, 640));
  window.dispatchEvent(new Event("resize"));
  const player = players[0]!;
  const host = player.parentElement!;
  const button = (label: string) => document.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`)!;
  expect(host.style.display).toBe("block");
  expect(document.getElementById("app")!.inert).toBe(true);

  button("暂停游戏").click(); await nextTick();
  expect(host.style.display).toBe("none");
  expect(host.inert).toBe(true);
  expect(suspend).toHaveBeenCalled();
  button("继续游戏").click(); await nextTick();
  expect(host.style.display).toBe("block");
  expect(resume).toHaveBeenCalled();
  player.focus();
  window.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", cancelable: true }));
  expect(document.activeElement).toBe(button("暂停游戏"));

  button("重新开始").click(); await nextTick();
  expect(host.style.display).toBe("none");
  window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }));
  await nextTick();
  expect(host.style.display).toBe("block");
  expect(createPlayer).toHaveBeenCalledOnce();

  window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }));
  await nextTick();
  expect(document.querySelector('[role="dialog"]')).toBeNull();
  expect(player.isConnected).toBe(true);
  expect(host.style.display).toBe("none");
  expect(document.getElementById("app")!.inert).toBe(false);
  expect(document.activeElement).toBe(launcher.element);

  await launcher.trigger("click"); await flushPromises();
  expect(createPlayer).toHaveBeenCalledOnce();
  expect(document.querySelector(".mota-player-host")).toBe(host);
  button("重新开始").click(); await nextTick();
  [...document.querySelectorAll<HTMLButtonElement>("button")].find(item => item.textContent === "确认重新开始")!.click();
  await flushPromises();
  expect(createPlayer).toHaveBeenCalledTimes(2);
  expect(player.isConnected).toBe(false);
  expect(players[1]!.isConnected).toBe(true);
  expect(document.querySelectorAll(".mota-player-host")).toHaveLength(1);
  expect(load).toHaveBeenCalledTimes(2);
});
