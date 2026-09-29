import { mount } from "@vue/test-utils";
import { defineComponent, nextTick, ref } from "vue";
import { afterEach, expect, it, vi } from "vitest";
import { useGameLoop } from "./useGameLoop";

afterEach(() => vi.unstubAllGlobals());
it("帧时间受限，暂停和失焦停止调度，恢复不补算时间，卸载释放循环", async () => {
  const callbacks = new Map<number, FrameRequestCallback>(); let sequence = 0;
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => { callbacks.set(++sequence, callback); return sequence; });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => callbacks.delete(id));
  const paused = ref(false), tick = vi.fn();
  const wrapper = mount(defineComponent({ setup() { useGameLoop(() => paused.value, tick); return () => null; } }));
  function frame(time: number) { const pending = [...callbacks.values()]; callbacks.clear(); pending.forEach(fn => fn(time)); }
  frame(100); frame(116); expect(tick).toHaveBeenLastCalledWith(.016);
  frame(3000); expect(tick).toHaveBeenLastCalledWith(.05);
  paused.value = true; await nextTick(); expect(callbacks.size).toBe(0);
  paused.value = false; await nextTick(); tick.mockClear(); frame(9000); expect(tick).not.toHaveBeenCalled(); frame(9020); expect(tick).toHaveBeenLastCalledWith(.02);
  window.dispatchEvent(new Event("blur")); expect(callbacks.size).toBe(0); window.dispatchEvent(new Event("focus")); expect(callbacks.size).toBe(1);
  wrapper.unmount(); expect(callbacks.size).toBe(0); window.dispatchEvent(new Event("focus")); expect(callbacks.size).toBe(0);
});
