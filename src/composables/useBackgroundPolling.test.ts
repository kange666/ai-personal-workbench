import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createBackgroundPolling } from "./useBackgroundPolling";

describe("后台轮询", () => {
  beforeEach(() => { vi.useFakeTimers(); Object.defineProperty(document,"visibilityState",{value:"visible",configurable:true}); });
  afterEach(() => { vi.useRealTimers(); });
  it("慢请求不重叠，完成后才安排下一轮，隐藏时暂停", async () => {
    const polling=createBackgroundPolling(); let finish!: () => void;
    const action=vi.fn().mockImplementationOnce(() => new Promise<void>(resolve => {finish=resolve;})).mockResolvedValue(undefined);
    polling.every("job",1000,action);
    await vi.advanceTimersByTimeAsync(6000); expect(action).toHaveBeenCalledTimes(1);
    finish(); await vi.advanceTimersByTimeAsync(1000); expect(action).toHaveBeenCalledTimes(2);
    Object.defineProperty(document,"visibilityState",{value:"hidden",configurable:true});
    await vi.advanceTimersByTimeAsync(3000); expect(action).toHaveBeenCalledTimes(2);
    Object.defineProperty(document,"visibilityState",{value:"visible",configurable:true});
    await vi.advanceTimersByTimeAsync(1000); expect(action).toHaveBeenCalledTimes(3);
    polling.stop(); await vi.advanceTimersByTimeAsync(3000); expect(action).toHaveBeenCalledTimes(3);
  });
  it("事件触发和定时触发共用同名进行中请求", async () => {
    const polling=createBackgroundPolling(); let finish!: () => void;
    const action=vi.fn(() => new Promise<void>(resolve => {finish=resolve;}));
    const one=polling.run("job",action); const two=polling.run("job",action);
    await Promise.resolve(); expect(action).toHaveBeenCalledTimes(1);
    finish(); await Promise.all([one,two]); polling.stop();
  });
});
