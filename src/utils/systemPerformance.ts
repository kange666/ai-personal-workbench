import type { GpuPerformance, SystemPerformanceSnapshot } from "../services/backend";

export interface SystemPerformanceHistoryPoint {
  capturedAt: string;
  cpu: number | null;
  memory: number | null;
  disk: number | null;
  integratedGpu: number | null;
  discreteGpu: number | null;
  networkDownload: number | null;
  networkUpload: number | null;
}

export function emptySystemPerformanceSnapshot(): SystemPerformanceSnapshot {
  return {
    available: false,
    capturedAt: "",
    cpuUsagePercent: null,
    cpuFrequencyMhz: null,
    memoryUsagePercent: null,
    memoryUsedBytes: null,
    memoryTotalBytes: null,
    memoryAvailableBytes: null,
    diskUsagePercent: null,
    diskReadBytesPerSecond: null,
    diskWriteBytesPerSecond: null,
    networkDownloadBytesPerSecond: null,
    networkUploadBytesPerSecond: null,
    gpus: [],
    message: "正在读取本机性能数据",
  };
}

export function findPerformanceGpu(
  snapshot: SystemPerformanceSnapshot,
  kind: "integrated" | "discrete",
): GpuPerformance | undefined {
  const direct = snapshot.gpus.find(gpu => gpu.kind === kind);
  if (direct) return direct;
  if (snapshot.gpus.length < 2) return undefined;
  const ordered = [...snapshot.gpus].sort((left, right) => left.index - right.index);
  return kind === "integrated" ? ordered[0] : ordered[ordered.length - 1];
}

export function appendPerformanceHistory(
  history: SystemPerformanceHistoryPoint[],
  snapshot: SystemPerformanceSnapshot,
  maximum = 60,
) {
  if (!snapshot.available || !snapshot.capturedAt) return history;
  if (history.at(-1)?.capturedAt === snapshot.capturedAt) return history;
  const integrated = findPerformanceGpu(snapshot, "integrated");
  const discrete = findPerformanceGpu(snapshot, "discrete");
  return [
    ...history,
    {
      capturedAt: snapshot.capturedAt,
      cpu: snapshot.cpuUsagePercent,
      memory: snapshot.memoryUsagePercent,
      disk: snapshot.diskUsagePercent,
      integratedGpu: integrated?.usagePercent ?? null,
      discreteGpu: discrete?.usagePercent ?? null,
      networkDownload: snapshot.networkDownloadBytesPerSecond,
      networkUpload: snapshot.networkUploadBytesPerSecond,
    },
  ].slice(-maximum);
}

export function formatPerformancePercent(value: number | null | undefined) {
  return value === null || value === undefined ? "--" : `${Math.round(value)}%`;
}

export function formatPerformanceBytes(value: number | null | undefined, digits = 1) {
  if (value === null || value === undefined || !Number.isFinite(value)) return "--";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let amount = Math.max(0, value);
  let unit = 0;
  while (amount >= 1024 && unit < units.length - 1) {
    amount /= 1024;
    unit += 1;
  }
  return `${amount.toFixed(unit === 0 ? 0 : digits)} ${units[unit]}`;
}

export function formatPerformanceRate(value: number | null | undefined, compact = false) {
  const formatted = formatPerformanceBytes(value, compact ? 0 : 1);
  if (formatted === "--") return formatted;
  return `${compact ? formatted.replace(" ", "") : formatted}/s`;
}
