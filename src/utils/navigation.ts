export interface WorkbenchNavigationItem {
  path: string;
  icon: string;
  label: string;
  vip?: boolean;
}

export const workbenchNavigationItems: WorkbenchNavigationItem[] = [
  { path:"/", icon:"home", label:"工作台" },
  { path:"/work-records", icon:"records", label:"工作记录" },
  { path:"/projects", icon:"projects", label:"项目资产" },
  { path:"/deployments", icon:"deployments", label:"发布中心" },
  { path:"/api-docs", icon:"api", label:"接口文档" },
  { path:"/calendar", icon:"calendar", label:"工作日历" },
  { path:"/reports", icon:"reports", label:"报告中心" },
  { path:"/testing", icon:"testing", label:"测试中心" },
  { path:"/tokens", icon:"tokens", label:"Token 分析" },
  { path:"/tapd", icon:"tapd", label:"TAPD 工作" },
  { path:"/tapd-automation", icon:"automation", label:"自动处理" },
  { path:"/content", icon:"content", label:"内容工坊", vip:true },
  { path:"/videos", icon:"videos", label:"视频中心", vip:true },
  { path:"/knowledge", icon:"knowledge", label:"知识库" },
];

export const navigationOrderChangedEvent = "workbench-navigation-order-changed";
const navigationOrderStorageKey = "workbench-navigation-order-v1";
const hiddenNavigationStorageKey = "workbench-navigation-hidden-v1";

function normalizeNavigationOrder(value: unknown): string[] {
  const validPaths = new Set(workbenchNavigationItems.map((item) => item.path));
  const result = Array.isArray(value)
    ? value.filter((path): path is string => typeof path === "string" && validPaths.has(path))
    : [];
  const unique = [...new Set(result)];
  for (const item of workbenchNavigationItems) {
    if (!unique.includes(item.path)) unique.push(item.path);
  }
  return unique;
}

export function loadNavigationOrder(): string[] {
  try {
    return normalizeNavigationOrder(JSON.parse(localStorage.getItem(navigationOrderStorageKey) || "[]"));
  } catch {
    return normalizeNavigationOrder([]);
  }
}

export function saveNavigationOrder(order: string[]): string[] {
  const normalized = normalizeNavigationOrder(order);
  localStorage.setItem(navigationOrderStorageKey, JSON.stringify(normalized));
  window.dispatchEvent(new CustomEvent(navigationOrderChangedEvent, { detail: normalized }));
  return normalized;
}

function normalizeHiddenNavigationPaths(value: unknown): string[] {
  const validPaths = new Set(workbenchNavigationItems.map((item) => item.path));
  return Array.isArray(value)
    ? [...new Set(value.filter((path): path is string => typeof path === "string" && validPaths.has(path)))]
    : [];
}

export function loadHiddenNavigationPaths(): string[] {
  try {
    return normalizeHiddenNavigationPaths(JSON.parse(localStorage.getItem(hiddenNavigationStorageKey) || "[]"));
  } catch {
    return [];
  }
}

export function saveHiddenNavigationPaths(paths: string[]): string[] {
  const normalized = normalizeHiddenNavigationPaths(paths);
  localStorage.setItem(hiddenNavigationStorageKey, JSON.stringify(normalized));
  window.dispatchEvent(new CustomEvent(navigationOrderChangedEvent));
  return normalized;
}

export function orderedNavigationItems(order: string[]): WorkbenchNavigationItem[] {
  const rank = new Map(normalizeNavigationOrder(order).map((path,index) => [path,index]));
  return [...workbenchNavigationItems].sort((left,right) => (rank.get(left.path) ?? 999) - (rank.get(right.path) ?? 999));
}
