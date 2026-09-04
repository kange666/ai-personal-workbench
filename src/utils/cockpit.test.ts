import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { COCKPIT_IDLE_MS, activityHeatLevel, cockpitIdleSettingsChangedEvent, cockpitIdleState, loadCockpitIdleMinutes, recentDateKeys, saveCockpitIdleMinutes } from "./cockpit";

beforeEach(() => localStorage.clear());
afterEach(() => { vi.restoreAllMocks(); localStorage.clear(); });

describe("cockpitIdleState", () => {
  it("空闲十分钟前不进入驾驶舱，并在最后十秒给出倒计时", () => {
    const start = Date.parse("2026-09-01T08:00:00.000Z");
    expect(cockpitIdleState(start, start + COCKPIT_IDLE_MS - 11_000)).toEqual({ open: false, warningSeconds: 0 });
    expect(cockpitIdleState(start, start + COCKPIT_IDLE_MS - 9_400)).toEqual({ open: false, warningSeconds: 10 });
  });

  it("达到十分钟后进入驾驶舱", () => {
    const start = Date.parse("2026-09-01T08:00:00.000Z");
    expect(cockpitIdleState(start, start + COCKPIT_IDLE_MS)).toEqual({ open: true, warningSeconds: 0 });
  });

  it("支持自定义屏保时间和永不自动进入", () => {
    const start = Date.parse("2026-09-01T08:00:00.000Z");
    expect(cockpitIdleState(start, start + 5 * 60 * 1_000, 5)).toEqual({ open: true, warningSeconds: 0 });
    expect(cockpitIdleState(start, start + 60 * 60 * 1_000, null)).toEqual({ open: false, warningSeconds: 0 });
  });
});

describe("驾驶舱屏保偏好", () => {
  it("默认十分钟，保存后可恢复并通知应用壳层", () => {
    expect(loadCockpitIdleMinutes()).toBe(10);
    const changed = vi.fn();
    window.addEventListener(cockpitIdleSettingsChangedEvent, changed);
    saveCockpitIdleMinutes(30);
    expect(loadCockpitIdleMinutes()).toBe(30);
    expect(changed).toHaveBeenCalledTimes(1);
    saveCockpitIdleMinutes(null);
    expect(loadCockpitIdleMinutes()).toBeNull();
    window.removeEventListener(cockpitIdleSettingsChangedEvent, changed);
  });

  it("旧值或不支持的值安全回退到十分钟", () => {
    localStorage.setItem("workbench-cockpit-idle-minutes-v1", "120");
    expect(loadCockpitIdleMinutes()).toBe(10);
    expect(() => saveCockpitIdleMinutes(120)).toThrow("不支持的驾驶舱屏保时间");
  });
});

describe("recentDateKeys", () => {
  it("活跃热力图严格生成包含当天的九十天", () => {
    const dates = recentDateKeys(90, new Date(2026, 8, 1, 12));
    expect(dates).toHaveLength(90);
    expect(dates[0]).toBe("2026-06-04");
    expect(dates.at(-1)).toBe("2026-09-01");
    expect(new Set(dates).size).toBe(90);
  });
});

describe("activityHeatLevel", () => {
  it("按相对强度稳定映射为五档", () => {
    expect([0, 1, 3, 6, 10].map(value => activityHeatLevel(value, 10))).toEqual([0, 1, 2, 3, 4]);
  });
});
