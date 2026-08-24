import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import TestReportDialog from "./TestReportDialog.vue";
import type { TestRun } from "../services/backend";

const { readTestArtifact, exportTestReportPdf } = vi.hoisted(() => ({
  readTestArtifact: vi.fn(),
  exportTestReportPdf: vi.fn(),
}));

vi.mock("../services/backend", async () => {
  const actual = await vi.importActual<typeof import("../services/backend")>("../services/backend");
  return { ...actual, readTestArtifact, exportTestReportPdf, isTauriRuntime: () => true };
});

const run: TestRun = {
  id: "run-1",
  menuId: "project:example",
  project: "example-project",
  projectPath: "C:/example-project",
  menuName: "示例页面",
  mode: "browser-style",
  status: "failed",
  startedAt: "2026-08-24T10:00:00+08:00",
  finishedAt: "2026-08-24T10:00:02+08:00",
  reportMarkdown: "# 测试报告",
  outputExcerpt: "1 failed",
  errorMessage: "搜索按钮没有响应",
  selectedScenarios: ["页面显示", "搜索操作"],
  scenarioResults: [
    { id: "passed", title: "页面显示", status: "passed", durationMs: 200, purpose: "确认页面可见", steps: ["进入页面"], checks: ["标题可见"], errorMessage: "", artifacts: [] },
    { id: "failed", title: "搜索操作", status: "failed", durationMs: 300, purpose: "确认搜索可用", steps: ["输入关键词", "点击搜索"], checks: ["出现请求", "列表更新"], errorMessage: "搜索按钮没有响应", artifacts: [{ name: "失败页面", path: "C:/example-project/test-results/failure.png", contentType: "image/png", kind: "screenshot" }] },
  ],
  artifacts: [{ name: "失败页面", path: "C:/example-project/test-results/failure.png", contentType: "image/png", kind: "screenshot" }],
  totalCount: 2,
  passedCount: 1,
  failedCount: 1,
  skippedCount: 0,
  durationMs: 500,
  exitCode: 1,
  environmentSummary: "Node + Playwright",
  cleanupStatus: "not-applicable",
};

function mountDialog() {
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: "/tasks", component: { template: "<div />" } }] });
  return mount(TestReportDialog, { props: { run, title: "示例页面 · 浏览器测试" }, global: { plugins: [router] } });
}

describe("TestReportDialog", () => {
  beforeEach(() => {
    readTestArtifact.mockReset().mockResolvedValue("data:image/png;base64,AAAA");
    exportTestReportPdf.mockReset().mockResolvedValue("C:/reports/example.pdf");
  });

  it("优先展示失败详情并加载对应页面截图", async () => {
    const wrapper = mountDialog();
    await flushPromises();
    expect(wrapper.find(".problem-section").text()).toContain("搜索操作");
    expect(wrapper.find(".problem-section").text()).toContain("搜索按钮没有响应");
    expect(wrapper.find(".problem-section").text()).toContain("点击搜索");
    expect(wrapper.find(".scenario-screenshots img").attributes("src")).toBe("data:image/png;base64,AAAA");
    expect(readTestArtifact).toHaveBeenCalledWith("run-1", "C:/example-project/test-results/failure.png");
  });

  it("把报告操作集中在标题栏最右侧并可导出 PDF", async () => {
    const wrapper = mountDialog();
    const header = wrapper.find(".test-report-v2 > header");
    expect(header.find(".report-header-actions").exists()).toBe(true);
    expect(header.find(".report-header-actions").text()).toContain("导出 PDF");
    await header.find(".report-header-actions .button").trigger("click");
    await flushPromises();
    expect(exportTestReportPdf).toHaveBeenCalledWith("run-1");
    expect(wrapper.text()).toContain("PDF 已保存：C:/reports/example.pdf");
  });
});
