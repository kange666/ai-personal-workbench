import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { afterEach, expect, it, vi } from "vitest";
import Mota50Game from "./Mota50Game.vue";
import { discardMota50Frame, mota50Channel } from "../../services/mota50Frame";
let wrapper: VueWrapper | null = null;
afterEach(() => { wrapper?.unmount(); wrapper = null; discardMota50Frame(); document.body.innerHTML = ""; vi.restoreAllMocks(); });
function message(frame: HTMLIFrameElement, type: string, origin = location.origin, source: Window | null = frame.contentWindow) {
  window.dispatchEvent(new MessageEvent("message", { source, origin, data: { channel: mota50Channel, type } }));
}
it("验证消息来源，加载后提供原生存读档，关闭与重开分别保留和重建实例", async () => {
  const panel = document.createElement("section"); panel.className = "rest-dialog"; document.body.appendChild(panel);
  wrapper = mount(Mota50Game, { props: { paused: false }, attachTo: panel });
  const frame = document.querySelector<HTMLIFrameElement>("iframe")!;
  const send = vi.spyOn(frame.contentWindow!, "postMessage");
  vi.spyOn(panel, "getBoundingClientRect").mockReturnValue(new DOMRect(0, 0, 800, 700));
  vi.spyOn(wrapper.get(".mota50-surface").element, "getBoundingClientRect").mockReturnValue(new DOMRect(20, 100, 640, 480));
  message(frame, "ready", "https://example.invalid"); await flushPromises();
  expect(wrapper.get("button").attributes("disabled")).toBeDefined();
  message(frame, "ready"); await flushPromises();
  expect(frame.parentElement!.style.display).toBe("block");
  await wrapper.findAll("button")[0]!.trigger("click");
  expect(send).toHaveBeenCalledWith({ channel: mota50Channel, type: "save" }, location.origin);
  await wrapper.findAll("button")[1]!.trigger("click");
  expect(send).toHaveBeenCalledWith({ channel: mota50Channel, type: "load" }, location.origin);
  await wrapper.setProps({ paused: true });
  expect(frame.parentElement!.style.display).toBe("none"); expect(frame.parentElement!.inert).toBe(true);
  await wrapper.setProps({ paused: false });
  expect(frame.parentElement!.style.display).toBe("block");
  wrapper.unmount(); wrapper = null;
  expect(frame.isConnected).toBe(true); expect(frame.parentElement!.style.display).toBe("none");
  wrapper = mount(Mota50Game, { props: { paused: false }, attachTo: panel });
  expect(document.querySelectorAll("iframe")).toHaveLength(1);
  message(frame, "ready"); await flushPromises();
  (wrapper.vm as unknown as { restart(): void }).restart();
  expect(frame.isConnected).toBe(false); expect(document.querySelectorAll("iframe")).toHaveLength(1);
});
it("加载期间暂停不会抢先恢复；游戏内 Esc/Tab 交给外层弹窗处理", async () => {
  wrapper = mount(Mota50Game, { props: { paused: true } });
  const frame = document.querySelector<HTMLIFrameElement>("iframe")!;
  const send = vi.spyOn(frame.contentWindow!, "postMessage");
  message(frame, "ready"); await flushPromises();
  expect(send).not.toHaveBeenCalledWith({ channel: mota50Channel, type: "resume" }, location.origin);
  const keydown = vi.fn(); window.addEventListener("keydown", keydown);
  try { message(frame, "escape"); message(frame, "tab"); expect(keydown.mock.calls.map(call => call[0].key)).toEqual(["Escape", "Tab"]); }
  finally { window.removeEventListener("keydown", keydown); }
});
it("就绪消息与关闭发生在同一轮更新时，不会在关闭后偷偷恢复游戏", async () => {
  wrapper = mount(Mota50Game, { props: { paused: false } });
  const frame = document.querySelector<HTMLIFrameElement>("iframe")!;
  const send = vi.spyOn(frame.contentWindow!, "postMessage");
  message(frame, "ready"); wrapper.unmount(); wrapper = null;
  await flushPromises();
  expect(send).not.toHaveBeenCalledWith({ channel: mota50Channel, type: "resume" }, location.origin);
  expect(frame.parentElement!.style.display).toBe("none");
});
