import type { TestRun, WorkbenchNotification } from "../services/backend";

export function latestTestsByProjectAndMode(runs: TestRun[]): TestRun[] {
  const latest = new Map<string, TestRun>();
  for (const run of [...runs].sort((left, right) => right.startedAt.localeCompare(left.startedAt))) {
    const project = (run.projectPath || run.project).replaceAll("\\", "/").replace(/\/$/, "").toLowerCase();
    const key = JSON.stringify([project, run.menuId, run.mode]);
    if (!latest.has(key)) latest.set(key, run);
  }
  return [...latest.values()];
}

export function notificationsChanged(before: WorkbenchNotification[], after: WorkbenchNotification[]): boolean {
  return JSON.stringify(before) !== JSON.stringify(after);
}
