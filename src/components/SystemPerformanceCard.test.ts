import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import SystemPerformanceCard from "./SystemPerformanceCard.vue";

const snapshot={available:true,capturedAt:"2026-09-07T10:00:00Z",cpuUsagePercent:38,cpuFrequencyMhz:3400,memoryUsagePercent:71,memoryUsedBytes:7_100,memoryTotalBytes:10_000,memoryAvailableBytes:2_900,diskUsagePercent:4,diskReadBytesPerSecond:1_024,diskWriteBytesPerSecond:2_048,networkDownloadBytesPerSecond:1_258_291,networkUploadBytesPerSecond:24_576,gpus:[{index:0,kind:"integrated" as const,name:"Intel Iris Xe",usagePercent:32,temperatureCelsius:null,dedicatedUsedBytes:null,dedicatedTotalBytes:null,sharedUsedBytes:100,sharedTotalBytes:200},{index:1,kind:"discrete" as const,name:"NVIDIA RTX 3050",usagePercent:12,temperatureCelsius:70,dedicatedUsedBytes:700,dedicatedTotalBytes:4_000,sharedUsedBytes:0,sharedTotalBytes:8_000}],message:""};
const history=[{capturedAt:"2026-09-07T10:00:00Z",cpu:30,memory:70,disk:3,networkDownload:1_000,networkUpload:500,integratedGpu:30,discreteGpu:10},{capturedAt:"2026-09-07T10:00:01Z",cpu:38,memory:71,disk:4,networkDownload:1_258_291,networkUpload:24_576,integratedGpu:32,discreteGpu:12}];

describe("SystemPerformanceCard",() => {
  it("按已确认的三乘二顺序展示，独显有进度条且磁盘只有百分比",async() => {
    const wrapper=mount(SystemPerformanceCard,{props:{snapshot,history,optimizationState:"idle",optimizationMessage:""}});
    expect(wrapper.findAll(".rail-performance-cell small").map(item => item.text())).toEqual(["CPU","内存","GPU 1 · 独显","网络","GPU 0 · 集显","磁盘"]);
    expect(wrapper.find(".rail-performance-cell.cpu svg").exists()).toBe(true);
    expect(wrapper.find(".rail-performance-cell.discrete > i").exists()).toBe(true);
    expect(wrapper.find(".rail-performance-cell.disk > i").exists()).toBe(false);
    expect(wrapper.find(".rail-performance-cell.disk b").text()).toBe("4%");
    await wrapper.get(".rail-performance-grid").trigger("click");
    expect(wrapper.emitted("open")).toHaveLength(1);
    await wrapper.get(".rail-performance-head button").trigger("click");
    expect(wrapper.emitted("optimize")).toHaveLength(1);
  });
});
