import { describe, expect, it } from "vitest";
import { COCKPIT_IDLE_MS, activityHeatLevel, cockpitIdleState, recentDateKeys } from "./cockpit";

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
