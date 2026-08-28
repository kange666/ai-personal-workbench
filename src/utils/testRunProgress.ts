import type { TestRun } from "../services/backend";

export interface TestRunProgressEstimate {
  percent: number;
  etaText: string;
  estimatedDurationMs: number;
}

const fallbackScenarioDuration: Record<TestRun["mode"], number> = {
  real: 60_000,
  "browser-style": 30_000,
  mock: 15_000,
  "source-style": 10_000,
};

function scenarioCount(run: TestRun) {
  return Math.max(run.totalCount || run.selectedScenarios.length, 1);
}

function median(values: number[]) {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2
    ? sorted[middle]
    : (sorted[middle - 1] + sorted[middle]) / 2;
}

function historicalScenarioDuration(run: TestRun, history: TestRun[]) {
  const successful = history
    .filter((item) => item.id !== run.id)
    .filter((item) => item.status === "passed" && item.durationMs > 0)
    .filter((item) => item.menuId === run.menuId && item.mode === run.mode)
    .filter((item) => item.projectPath.toLocaleLowerCase() === run.projectPath.toLocaleLowerCase())
    .sort((left, right) => right.startedAt.localeCompare(left.startedAt))
    .slice(0, 5)
    .map((item) => item.durationMs / scenarioCount(item))
    .filter((value) => Number.isFinite(value) && value > 0);

  return successful.length ? median(successful) : fallbackScenarioDuration[run.mode];
}

function formatRemainingTime(milliseconds: number) {
  const seconds = Math.max(1, Math.ceil(milliseconds / 1_000));
  if (seconds < 60) return `${Math.ceil(seconds / 5) * 5}秒`;
  const minutes = Math.floor(seconds / 60);
  const remainingSeconds = Math.ceil((seconds % 60) / 10) * 10;
  if (minutes < 60) return `${minutes}分${remainingSeconds ? `${remainingSeconds}秒` : ""}`;
  const hours = Math.floor(minutes / 60);
  const remainingMinutes = minutes % 60;
  return `${hours}小时${remainingMinutes ? `${remainingMinutes}分` : ""}`;
}

export function estimateTestRunProgress(
  run: TestRun,
  history: TestRun[],
  now = Date.now(),
): TestRunProgressEstimate {
  if (run.status === "queued") {
    return { percent: 0, etaText: "等待开始", estimatedDurationMs: 0 };
  }

  const total = scenarioCount(run);
  const completed = Math.min(total, run.passedCount + run.failedCount + run.skippedCount);
  const startedAt = Date.parse(run.startedAt);
  const elapsed = Number.isFinite(startedAt) ? Math.max(0, now - startedAt) : 0;
  const historicalPerScenario = historicalScenarioDuration(run, history);
  const actualPerScenario = completed > 0 ? elapsed / completed : historicalPerScenario;
  const perScenario = Math.min(Math.max(actualPerScenario, 3_000), 10 * 60_000);
  const estimatedDurationMs = Math.min(Math.max(perScenario * total, 10_000), 2 * 60 * 60_000);
  const percentFromScenarios = completed > 0 ? (completed / total) * 100 : 0;
  const percentFromTime = estimatedDurationMs > 0 ? (elapsed / estimatedDurationMs) * 100 : 0;
  const percent = Math.min(95, Math.max(elapsed > 0 ? 2 : 0, Math.round(
    completed > 0 ? percentFromScenarios : percentFromTime,
  )));
  const remaining = completed > 0
    ? perScenario * Math.max(total - completed, 0)
    : estimatedDurationMs - elapsed;

  return {
    percent,
    etaText: remaining > 0 ? `预计剩余 ${formatRemainingTime(remaining)}` : "已超过预估，正在收尾",
    estimatedDurationMs,
  };
}
