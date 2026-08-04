import { describe, expect, it } from "vitest";
import { getAlmanac, lunarDayLabel } from "./almanac";

describe("本地黄历", () => {
  it("生成指定日期的完整宜忌和干支信息", () => {
    const detail = getAlmanac("2026-08-04");
    expect(detail.date).toBe("2026-08-04");
    expect(detail.lunarDate).toContain("农历");
    expect(detail.ganZhi).toContain("丙午年");
    expect(detail.yi.length).toBeGreaterThan(0);
    expect(detail.ji.length).toBeGreaterThan(0);
    expect(detail.clash).toContain("龙");
  });

  it("为日历格生成简短农历标签", () => {
    expect(lunarDayLabel("2026-08-04")).toBe("廿二");
  });
});
