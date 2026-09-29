import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as backend from "../services/backend";
import WorkRecordsView from "./WorkRecordsView.vue";
vi.mock("../services/backend");
vi.mock("../stores/workbench", () => ({useWorkbenchStore:()=>({tasks:[]})}));
let wrapper: VueWrapper;
const date=new Date().toLocaleDateString("sv-SE");
const summary={totalMinutes:0,estimatedMinutes:0,manualMinutes:0,byProject:[],byType:[],daily:[]} as unknown as backend.WorkSummary;
const activity={date,messageCount:0,publishCount:2,publishesSucceeded:1,conversationCount:0,archivedConversationCount:0,gitCommits:0,totalTokens:0,testRuns:0,testsPassed:0,taskActivityCount:0,knowledgeCount:0} as backend.DailyActivity;
beforeEach(() => {
  vi.resetAllMocks(); Object.defineProperty(document,"visibilityState",{value:"visible",configurable:true});
  vi.mocked(backend.isTauriRuntime).mockReturnValue(true);
  vi.mocked(backend.getHistoryCoverage).mockResolvedValue({firstDate:date} as backend.HistoryCoverage);
  vi.mocked(backend.getWorkSummary).mockResolvedValue(summary);
  vi.mocked(backend.getDailyActivity).mockResolvedValue([activity]);
  for(const command of [backend.listReports,backend.listWorkSessions,backend.listConversationMetrics,backend.listKnowledge,backend.listTestMenus,backend.listTestRuns]) vi.mocked(command).mockResolvedValue([]);
});
afterEach(() => wrapper?.unmount());
async function render() {
  const router=createRouter({history:createMemoryHistory(),routes:[{path:"/:pathMatch(.*)*",component:{template:"<div/>"}}]});
  await router.push("/work-records");
  wrapper=mount(WorkRecordsView,{global:{plugins:[router],stubs:{WorkTimeDrawer:true}}});
  await flushPromises(); return router;
}
describe("工作记录按需读取", () => {
  it("初次只读，保留无工时的发布日；人工刷新才重算", async () => {
    await render();
    expect(backend.getWorkSummary).toHaveBeenLastCalledWith(expect.any(String),expect.any(String),false);
    expect(wrapper.get(".work-record-card").text()).toContain("发布 2 次 · 成功 1 次");
    expect(wrapper.get(".work-record-card header strong").text()).toBe("0分钟");
    await wrapper.get(".page-header button").trigger("click"); await flushPromises();
    expect(backend.getWorkSummary).toHaveBeenLastCalledWith(expect.any(String),expect.any(String),true);
  });
  it("报告读取失败仍显示活动，并给出错误而非伪装全空", async () => {
    vi.mocked(backend.listReports).mockRejectedValueOnce(new Error("busy"));
    await render();
    expect(wrapper.get(".scan-message.error").text()).toContain("报告");
    expect(wrapper.get(".work-record-card").text()).toContain("发布 2 次");
  });
  it("加载中多个来源更新只追加一次重算，不并发整页查询", async () => {
    let finish!: (value:backend.WorkSummary)=>void;
    vi.mocked(backend.getWorkSummary).mockReturnValueOnce(new Promise(resolve=>{finish=resolve;}));
    await render();
    window.dispatchEvent(new CustomEvent("workbench-codex-data-updated")); window.dispatchEvent(new CustomEvent("workbench-codex-data-updated"));
    expect(backend.getWorkSummary).toHaveBeenCalledTimes(1);
    finish(summary); await flushPromises();
    expect(backend.getWorkSummary).toHaveBeenCalledTimes(2);
    expect(backend.getWorkSummary).toHaveBeenLastCalledWith(expect.any(String),expect.any(String),true);
  });
});
