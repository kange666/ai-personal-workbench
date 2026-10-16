import { mount } from "@vue/test-utils";
import { defineComponent, markRaw, nextTick } from "vue";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import GameDialog from "./GameDialog.vue";
import type { RestGameId } from "../../utils/rest/catalog";
const reset = vi.fn();
const FakeGame = markRaw(defineComponent({ props: { paused: Boolean }, setup(_props, { expose }) { expose({ restart: reset }); }, template: '<div tabindex="0" data-game-focus><button>游戏动作</button><button tabindex="-1">棋盘内部按钮</button><span>{{ paused }}</span></div>' }));
let wrapper: ReturnType<typeof mount>;
beforeEach(() => { document.body.innerHTML = '<div id="app"><button id="launcher">打开</button></div>'; reset.mockReset(); });
afterEach(() => { wrapper?.unmount(); document.body.innerHTML = ""; vi.restoreAllMocks(); });
const button = (label: string) => document.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`)!;
async function render(game: RestGameId = "wiring") { document.getElementById("launcher")!.focus(); wrapper = mount(GameDialog, { props: { game, component: FakeGame }, attachTo: document.body }); await nextTick(); }
it("打开锁住背景焦点、关闭恢复入口焦点和背景交互", async () => {
  await render(); expect(document.getElementById("app")!.inert).toBe(true); expect(document.activeElement?.hasAttribute("data-game-focus")).toBe(true);
  button("保存并关闭").click(); expect(wrapper.emitted("close")).toHaveLength(1);
  wrapper.unmount(); expect(document.getElementById("app")!.inert).toBe(false); expect(document.activeElement?.id).toBe("launcher");
});
it("失焦暂停，恢复及重开确认均正常", async () => {
  await render(); window.dispatchEvent(new Event("blur")); await nextTick(); expect(document.querySelector(".rest-pause-cover")!.textContent).toContain("游戏已暂停");
  button("继续游戏").click(); await nextTick(); expect(document.querySelector(".rest-pause-cover")).toBeNull();
  button("重新开始").click(); await nextTick(); expect(reset).not.toHaveBeenCalled();
  [...document.querySelectorAll<HTMLButtonElement>("button")].find(e => e.textContent === "确认重新开始")!.click(); await nextTick(); await nextTick(); expect(reset).toHaveBeenCalledOnce();
});
it("塔防切换窗口不自动暂停，手动暂停和隐藏暂停仍有效", async () => {
  await render("defense");
  window.dispatchEvent(new Event("blur")); await nextTick();
  expect(document.querySelector(".rest-pause-cover")).toBeNull();
  expect(wrapper.findComponent(FakeGame).props("paused")).toBe(false);
  button("暂停游戏").click(); await nextTick();
  window.dispatchEvent(new Event("blur")); window.dispatchEvent(new Event("focus")); await nextTick();
  expect(wrapper.findComponent(FakeGame).props("paused")).toBe(true);
  button("继续游戏").click(); await nextTick();
  expect(document.querySelector(".rest-pause-cover")).toBeNull();
  vi.spyOn(document, "hidden", "get").mockReturnValue(true);
  document.dispatchEvent(new Event("visibilitychange")); await nextTick();
  expect(wrapper.findComponent(FakeGame).props("paused")).toBe(true);
  expect(document.querySelector(".rest-pause-cover")!.textContent).toContain("游戏已暂停");
});
it("Esc先取消重开，再关闭；Tab不会跳入背景", async () => {
  await render(); button("重新开始").click(); await nextTick(); window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })); await nextTick(); expect(wrapper.emitted("close")).toBeUndefined();
  const gameButton = document.querySelector<HTMLButtonElement>("[data-game-focus] button")!; gameButton.focus(); window.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", cancelable: true })); expect(document.activeElement).toBe(button("暂停游戏"));
  window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })); expect(wrapper.emitted("close")).toHaveLength(1);
});
it("工作台快捷键不会在游戏背后打开搜索或快捷记录", async () => {
  await render(); const background = vi.fn(); window.addEventListener("keydown", background);
  window.dispatchEvent(new KeyboardEvent("keydown", { key:"k",ctrlKey:true }));
  window.dispatchEvent(new KeyboardEvent("keydown", { key:" ",code:"Space",ctrlKey:true,shiftKey:true }));
  expect(background).not.toHaveBeenCalled(); window.removeEventListener("keydown", background);
});
it("魔塔播放器位于持久节点中时，Tab与Shift+Tab仍限制在游戏弹窗内", async () => {
  const MotaStub = markRaw(defineComponent({ template: '<div tabindex="0" data-game-focus>魔塔区域</div>' }));
  wrapper = mount(GameDialog, { props: { game: "mota", component: MotaStub }, attachTo: document.body });
  await nextTick();
  const host = document.createElement("div"); host.className = "mota-player-host";
  const player = document.createElement("button"); host.appendChild(player); document.body.appendChild(host);
  player.focus(); window.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", cancelable: true }));
  expect(document.activeElement).toBe(button("暂停游戏"));
  player.focus(); window.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", shiftKey: true, cancelable: true }));
  expect(document.activeElement).toBe(button("关闭游戏"));
  expect(document.querySelector(".rest-dialog-mota")?.textContent).toContain("经典 21 层 / 50 层");
  host.remove();
});
it("焦点进入同一游戏文档时保持运行，整个窗口失焦时暂停", async () => {
  await render();
  await wrapper.setProps({ game: "mota" });
  const frame = document.createElement("iframe"); document.body.appendChild(frame); frame.focus();
  const focused = vi.spyOn(document, "hasFocus").mockReturnValue(true);
  window.dispatchEvent(new Event("blur")); await nextTick();
  expect(document.querySelector(".rest-pause-cover")).toBeNull();
  focused.mockReturnValue(false);
  window.dispatchEvent(new Event("blur")); await nextTick();
  expect(document.querySelector(".rest-pause-cover")).not.toBeNull();
  focused.mockRestore();
});
