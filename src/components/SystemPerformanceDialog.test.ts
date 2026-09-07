import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import SystemPerformanceDialog from "./SystemPerformanceDialog.vue";

const snapshot={available:true,capturedAt:"2026-09-07T10:00:00Z",cpuUsagePercent:38,cpuFrequencyMhz:3400,memoryUsagePercent:71,memoryUsedBytes:7_100,memoryTotalBytes:10_000,memoryAvailableBytes:2_900,diskUsagePercent:4,diskReadBytesPerSecond:1_024,diskWriteBytesPerSecond:2_048,networkDownloadBytesPerSecond:1_258_291,networkUploadBytesPerSecond:24_576,gpus:[{index:0,kind:"integrated" as const,name:"Intel Iris Xe",usagePercent:32,temperatureCelsius:null,dedicatedUsedBytes:null,dedicatedTotalBytes:null,sharedUsedBytes:100,sharedTotalBytes:200},{index:1,kind:"discrete" as const,name:"NVIDIA RTX 3050",usagePercent:12,temperatureCelsius:70,dedicatedUsedBytes:700,dedicatedTotalBytes:4_000,sharedUsedBytes:0,sharedTotalBytes:8_000}],message:""};
const history=Array.from({length:60},(_,index)=>({capturedAt:`2026-09-07T10:00:${String(index).padStart(2,"0")}Z`,cpu:25+index%20,memory:70,disk:4,networkDownload:1_000+index*100,networkUpload:500+index*10,integratedGpu:30,discreteGpu:12}));

describe("SystemPerformanceDialog",() => {
  it("展示双显卡详情并可切换网络图表和关闭",async() => {
    const wrapper=mount(SystemPerformanceDialog,{props:{open:true,snapshot,history},global:{stubs:{Teleport:true,Transition:false}}});
    expect(wrapper.find(".performance-summary").text()).toContain("GPU 0 · 集显32%");
    expect(wrapper.find(".performance-summary").text()).toContain("GPU 1 · 独显12%");
    expect(wrapper.findAll(".performance-chart polyline")).toHaveLength(5);
    await wrapper.findAll(".performance-chart-panel nav button")[1].trigger("click");
    expect(wrapper.find(".performance-chart-panel h3").text()).toContain("网络吞吐");
    expect(wrapper.findAll(".performance-chart polyline")).toHaveLength(2);
    await wrapper.get(".performance-dialog-header .icon-button").trigger("click");
    expect(wrapper.emitted("close")).toHaveLength(1);
  });
});
