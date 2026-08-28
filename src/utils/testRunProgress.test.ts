import { describe, expect, it } from "vitest";
import type { TestRun } from "../services/backend";
import { estimateTestRunProgress } from "./testRunProgress";

function testRun(overrides: Partial<TestRun> = {}): TestRun {
  return {
    id: "run-current",
    menuId: "post",
    project: "client",
    projectPath: "F:/TB-project/client",
    menuName: "岗位管理",
    mode: "real",
    status: "running",
    startedAt: "2026-08-28T08:00:00.000Z",
    reportMarkdown: "",
    outputExcerpt: "",
    errorMessage: "",
    selectedScenarios: ["open", "query"],
    scenarioResults: [],
    artifacts: [],
    totalCount: 2,
    passedCount: 0,
    failedCount: 0,
    skippedCount: 0,
    durationMs: 0,
    environmentSummary: "",
    cleanupStatus: "pending" as TestRun["cleanupStatus"],
    ...overrides,
  };
}

describe("estimateTestRunProgress", () => {
  it("优先使用同项目同菜单的历史单场景耗时", () => {
    const current = testRun();
    const history = [testRun({
      id: "run-history",
      status: "passed",
      startedAt: "2026-08-27T08:00:00.000Z",
      totalCount: 4,
      durationMs: 120_000,
    })];

    const result = estimateTestRunProgress(current, history, Date.parse("2026-08-28T08:00:30.000Z"));

    expect(result.estimatedDurationMs).toBe(60_000);
    expect(result.percent).toBe(50);
    expect(result.etaText).toBe("预计剩余 30秒");
  });

  it("没有历史记录时按测试模式给出保守预估", () => {
    const result = estimateTestRunProgress(
      testRun(),
      [],
      Date.parse("2026-08-28T08:00:30.000Z"),
    );

    expect(result.estimatedDurationMs).toBe(120_000);
    expect(result.percent).toBe(25);
    expect(result.etaText).toBe("预计剩余 1分30秒");
  });

  it("超过预估后停在 95% 并明确提示正在收尾", () => {
    const result = estimateTestRunProgress(
      testRun({ totalCount: 1, selectedScenarios: ["open"] }),
      [],
      Date.parse("2026-08-28T08:02:00.000Z"),
    );

    expect(result.percent).toBe(95);
    expect(result.etaText).toBe("已超过预估，正在收尾");
  });

  it("排队任务不提前显示进度", () => {
    const result = estimateTestRunProgress(testRun({ status: "queued" }), [], Date.now());

    expect(result).toEqual({ percent: 0, etaText: "等待开始", estimatedDurationMs: 0 });
  });
});
