import { describe, expect, it } from "vitest";
import type { SystemPerformanceSnapshot } from "../services/backend";
import { appendPerformanceHistory, findPerformanceGpu, formatPerformanceRate } from "./systemPerformance";

function snapshot(overrides:Partial<SystemPerformanceSnapshot> = {}):SystemPerformanceSnapshot {
  return {
    available:true,
    capturedAt:"2026-09-07T10:00:00Z",
    cpuUsagePercent:36,
    cpuFrequencyMhz:3400,
    memoryUsagePercent:70,
    memoryUsedBytes:7_000,
    memoryTotalBytes:10_000,
    memoryAvailableBytes:3_000,
    diskUsagePercent:4,
    diskReadBytesPerSecond:1_024,
    diskWriteBytesPerSecond:2_048,
    networkDownloadBytesPerSecond:1_024,
    networkUploadBytesPerSecond:512,
    gpus:[
      {index:0,kind:"integrated",name:"Intel",usagePercent:20,temperatureCelsius:null,dedicatedUsedBytes:null,dedicatedTotalBytes:null,sharedUsedBytes:100,sharedTotalBytes:200},
      {index:1,kind:"discrete",name:"NVIDIA",usagePercent:12,temperatureCelsius:68,dedicatedUsedBytes:700,dedicatedTotalBytes:4_000,sharedUsedBytes:0,sharedTotalBytes:8_000},
    ],
    message:"",
    ...overrides,
  };
}

describe("systemPerformance",() => {
  it("按显卡类型选择集显和独显",() => {
    expect(findPerformanceGpu(snapshot(),"integrated")?.name).toBe("Intel");
    expect(findPerformanceGpu(snapshot(),"discrete")?.name).toBe("NVIDIA");
  });

  it("追加采样并只保留指定数量的最近记录",() => {
    let history=appendPerformanceHistory([],snapshot(),2);
    history=appendPerformanceHistory(history,snapshot({capturedAt:"2026-09-07T10:00:01Z",cpuUsagePercent:40}),2);
    history=appendPerformanceHistory(history,snapshot({capturedAt:"2026-09-07T10:00:02Z",cpuUsagePercent:45}),2);
    expect(history.map(item => item.cpu)).toEqual([40,45]);
    expect(history[1].discreteGpu).toBe(12);
  });

  it("紧凑速率不带空格",() => {
    expect(formatPerformanceRate(1_048_576,true)).toBe("1MB/s");
    expect(formatPerformanceRate(null,true)).toBe("--");
  });
});
