import { onBeforeUnmount } from "vue";

/** 同名操作同时最多运行一次；窗口隐藏时暂停，恢复可见后重新排期。 */
export function createBackgroundPolling() {
  const active = new Map<string, Promise<unknown>>();
  const timers = new Set<number>();
  let stopped = false;
  async function run<T>(name: string, action: () => Promise<T>): Promise<T | undefined> {
    if (stopped || document.visibilityState === "hidden") return undefined;
    const pending = active.get(name);
    if (pending) return pending as Promise<T>;
    const promise = Promise.resolve().then(action);
    active.set(name, promise);
    try { return await promise; }
    finally { if (active.get(name) === promise) active.delete(name); }
  }
  function every(name: string, interval: number, action: () => Promise<unknown>, onError: (cause: unknown) => void = console.error) {
    const schedule = () => {
      if (stopped) return;
      const timer = window.setTimeout(async () => {
        timers.delete(timer);
        try { await run(name, action); } catch (cause) { onError(cause); }
        schedule();
      }, interval);
      timers.add(timer);
    };
    schedule();
  }
  function stop() { stopped = true; for (const timer of timers) window.clearTimeout(timer); timers.clear(); }
  return { run, every, stop };
}

export function useBackgroundPolling() {
  const polling = createBackgroundPolling();
  onBeforeUnmount(polling.stop);
  return polling;
}
