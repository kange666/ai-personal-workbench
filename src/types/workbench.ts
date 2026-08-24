export type ThemeMode = "command" | "warm";
export type TaskScope = "day" | "week" | "project";
export type TaskStatus = "todo" | "doing" | "done" | "blocked" | "overdue" | "cancelled" | "draft";
export type TaskPriority = "P0" | "P1" | "P2";
export type TaskSource = "manual" | "conversation" | "test" | "report" | "ai" | "inbox";

export interface WorkTask {
  id: string;
  title: string;
  project: string;
  scope: TaskScope;
  status: TaskStatus;
  priority: TaskPriority;
  plannedDate?: string;
  weekStart?: string;
  startDate?: string;
  endDate?: string;
  progress: number;
  note: string;
  source: TaskSource;
  sourceId?: string;
  createdAt: string;
  updatedAt: string;
  completedAt?: string;
}

export interface TaskDraft {
  title: string;
  project: string;
  scope: TaskScope;
  priority: TaskPriority;
  plannedDate: string;
  weekStart: string;
  startDate: string;
  endDate: string;
  progress: number;
  note: string;
}
