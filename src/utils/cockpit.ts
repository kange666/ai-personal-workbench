export const COCKPIT_IDLE_MS = 10 * 60 * 1_000;
export const COCKPIT_WARNING_MS = 10 * 1_000;

export interface CockpitIdleState {
  open: boolean;
  warningSeconds: number;
}

/** 根据最后一次有效操作计算屏保状态，后台数据刷新不会影响这个时间。 */
export function cockpitIdleState(lastActivityAt: number, now = Date.now()): CockpitIdleState {
  const elapsed = Math.max(0, now - lastActivityAt);
  if (elapsed >= COCKPIT_IDLE_MS) return { open: true, warningSeconds: 0 };
  const remaining = COCKPIT_IDLE_MS - elapsed;
  return {
    open: false,
    warningSeconds: remaining <= COCKPIT_WARNING_MS ? Math.max(1, Math.ceil(remaining / 1_000)) : 0,
  };
}

export function localDateIso(date: Date) {
  return date.toLocaleDateString("sv-SE");
}

/** 返回包含结束日在内的连续日期，驾驶舱活跃热力图当前固定使用 90 天。 */
export function recentDateKeys(days: number, endDate = new Date()) {
  return Array.from({ length: days }, (_, index) => {
    const date = new Date(endDate);
    date.setHours(12, 0, 0, 0);
    date.setDate(date.getDate() - (days - index - 1));
    return localDateIso(date);
  });
}

export function activityHeatLevel(value: number, maximum: number) {
  if (value <= 0 || maximum <= 0) return 0;
  const ratio = value / maximum;
  if (ratio <= 0.25) return 1;
  if (ratio <= 0.5) return 2;
  if (ratio <= 0.75) return 3;
  return 4;
}

export function compactCockpitNumber(value: number) {
  if (value >= 1_000_000_000) return `${(value / 1_000_000_000).toFixed(2)}B`;
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(2)}M`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`;
  return String(Math.round(value));
}

export function cockpitHours(minutes: number) {
  if (!minutes) return "0h";
  const hours = minutes / 60;
  return `${hours >= 10 ? hours.toFixed(1) : hours.toFixed(hours % 1 ? 1 : 0)}h`;
}
