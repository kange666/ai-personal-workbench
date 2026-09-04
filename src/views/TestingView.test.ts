import { flushPromises, mount } from "@vue/test-utils";
import { createPinia } from "pinia";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const backend = vi.hoisted(() => ({
  startTestCaseGeneration: vi.fn(),
  getTestCaseGeneration: vi.fn(),
  listTestCaseGenerations: vi.fn(),
}));

vi.mock("../services/backend", () => ({
  cancelTestRun: vi.fn(),
  deleteTask: vi.fn(),
  getTestCaseGeneration: backend.getTestCaseGeneration,
  getTestRun: vi.fn(),
  isTauriRuntime: () => true,
  listFeatureParity: vi.fn(async () => []),
  listTasks: vi.fn(async () => []),
  listTestCaseGenerations: backend.listTestCaseGenerations,
  listTestMenus: vi.fn(async () => [{
    id:"system-post", project:"client", projectPath:"F:/TB-project/client", projectKind:"vue",
    name:"岗位管理", route:"/system/post", sourcePath:"src/views/system/post/index.vue",
    hasCaseFile:true, canCreateCaseFile:false,
    capabilities:{mock:true,realApi:true,sourceStyle:true,browserStyle:true}, tested:false,
  }]),
  listTestProjects: vi.fn(async () => [{
    path:"F:/TB-project/client", name:"client", projectKind:"vue", caseCount:1, pageCount:1,
    capabilities:{mock:true,realApi:true,sourceStyle:true,browserStyle:true}, warnings:[],
  }]),
  listTestRuns: vi.fn(async () => []),
  listTestScenarios: vi.fn(async (_path:string, _menu:string, mode:string) => [{ id:`${mode}-1`, title:"页面基础区域正常显示", description:"确认页面可用", mode, defaultSelected:true }]),
  listTestSuites: vi.fn(async () => [{ id:"common-real", name:"公共通用用例", description:"只读检查", kind:"common", readOnly:true }]),
  listToolchains: vi.fn(async () => ({installations:[],conflicts:[]})),
  listWeeklyAudits: vi.fn(async () => []),
  preflightTest: vi.fn(),
  readTestReport: vi.fn(),
  recommendTestsFromGit: vi.fn(async () => []),
  runWeeklyAudit: vi.fn(),
  saveFeatureParityReview: vi.fn(),
  saveTask: vi.fn(),
  scanToolchains: vi.fn(),
  startTestCaseGeneration: backend.startTestCaseGeneration,
  startTestRun: vi.fn(),
  syncFeatureParity: vi.fn(),
}));

import TestingView from "./TestingView.vue";

describe("TestingView 专属用例后台生成", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    window.localStorage.clear();
    window.sessionStorage.clear();
    backend.listTestCaseGenerations.mockResolvedValue([]);
    backend.startTestCaseGeneration.mockResolvedValue({
      id:"generation-1", projectPath:"F:\\TB-project\\client", menuId:"system-post", menuName:"岗位管理",
      status:"queued", progressPercent:3, progressMessage:"正在准备 Codex CLI 生成任务", errorMessage:"", createdAt:"2026-09-04T08:00:00Z",
    });
    backend.getTestCaseGeneration.mockResolvedValue({
      id:"generation-1", projectPath:"F:\\TB-project\\client", menuId:"system-post", menuName:"岗位管理",
      status:"running", progressPercent:38, progressMessage:"正在检查页面源码和接口定义", errorMessage:"", createdAt:"2026-09-04T08:00:00Z",
    });
  });

  afterEach(() => vi.useRealTimers());

  it("点击生成后立即关闭弹框，页面仍可继续操作", async () => {
    const router = createRouter({ history:createMemoryHistory(), routes:[{path:"/testing",component:TestingView}] });
    await router.push("/testing");
    await router.isReady();
    const wrapper = mount(TestingView, { global:{ plugins:[createPinia(),router], stubs:{TestReportDialog:true} } });
    await flushPromises();

    await wrapper.get(".test-table .button.primary").trigger("click");
    await flushPromises();
    await wrapper.get(".test-config-body > label select").setValue("real");
    await flushPromises();
    expect(wrapper.find(".dedicated-case-empty").exists()).toBe(true);

    await wrapper.get(".dedicated-case-empty button").trigger("click");
    await flushPromises();

    expect(backend.startTestCaseGeneration).toHaveBeenCalledWith("F:/TB-project/client", "system-post");
    expect(wrapper.find(".test-config-dialog").exists()).toBe(false);
    expect(wrapper.get(".testing-tabs button").attributes("disabled")).toBeUndefined();
    expect(wrapper.text()).toContain("右侧“正在执行”会显示实时进度");
    wrapper.unmount();
  });
});
