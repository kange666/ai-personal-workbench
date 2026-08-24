import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import TestReportDialog from "./TestReportDialog.vue";
import type { TestRun } from "../services/backend";

const { readTestArtifact, exportTestReportMarkdown, exportTestReportPdf, getExistingTestReportPdf, openTestReportPdf } = vi.hoisted(() => ({
  readTestArtifact: vi.fn(),
  exportTestReportMarkdown: vi.fn(),
  exportTestReportPdf: vi.fn(),
  getExistingTestReportPdf: vi.fn(),
  openTestReportPdf: vi.fn(),
}));

vi.mock("../services/backend", async () => {
  const actual = await vi.importActual<typeof import("../services/backend")>("../services/backend");
  return { ...actual, readTestArtifact, exportTestReportMarkdown, exportTestReportPdf, getExistingTestReportPdf, openTestReportPdf, isTauriRuntime: () => true };
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

function mountDialog(props: { run?: TestRun | null; fallbackMarkdown?: string } = {}) {
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: "/tasks", component: { template: "<div />" } }] });
  return mount(TestReportDialog, { props: { run: props.run === undefined ? run : props.run, title: "示例页面 · 浏览器测试", fallbackMarkdown: props.fallbackMarkdown }, global: { plugins: [router] } });
}

describe("TestReportDialog", () => {
  beforeEach(() => {
    readTestArtifact.mockReset().mockResolvedValue("data:image/png;base64,AAAA");
    exportTestReportMarkdown.mockReset().mockResolvedValue("C:/reports/example.md");
    exportTestReportPdf.mockReset().mockResolvedValue("C:/reports/example.pdf");
    getExistingTestReportPdf.mockReset().mockResolvedValue(null);
    openTestReportPdf.mockReset().mockResolvedValue(undefined);
  });

  it("优先展示失败详情并加载对应页面截图", async () => {
    const wrapper = mountDialog();
    await flushPromises();
    expect(wrapper.find(".problem-section").text()).toContain("搜索操作");
    expect(wrapper.find(".problem-section").text()).toContain("搜索按钮没有响应");
    expect(wrapper.find(".problem-section").text()).toContain("点击搜索");
    expect(wrapper.find(".scenario-screenshots img").attributes("src")).toBe("data:image/png;base64,AAAA");
    expect(readTestArtifact).toHaveBeenCalledWith("run-1", "C:/example-project/test-results/failure.png");
    await wrapper.find(".screenshot-preview-button").trigger("click");
    expect(wrapper.find(".screenshot-lightbox img").attributes("src")).toBe("data:image/png;base64,AAAA");
  });

  it("把报告操作集中在标题栏最右侧并可导出 PDF", async () => {
    const wrapper = mountDialog();
    await flushPromises();
    const header = wrapper.find(".test-report-v2 > header");
    expect(header.find(".report-header-actions").exists()).toBe(true);
    expect(header.find(".report-header-actions").text()).toContain("导出 PDF");
    const pdfButton = header.find('[data-testid="pdf-action"]');
    await pdfButton.trigger("click");
    await flushPromises();
    expect(exportTestReportPdf).toHaveBeenCalledWith("run-1");
    expect(wrapper.text()).toContain("PDF 已保存：C:/reports/example.pdf");
    expect(pdfButton.text()).toBe("打开 PDF");
    await pdfButton.trigger("click");
    await flushPromises();
    expect(openTestReportPdf).toHaveBeenCalledWith("C:/reports/example.pdf");
  });

  it("可把当前测试报告导出为 Markdown 文件", async () => {
    const wrapper = mountDialog();
    await flushPromises();
    const markdownButton = wrapper.find('[data-testid="export-markdown"]');
    expect(markdownButton.text()).toBe("导出 MD");
    await markdownButton.trigger("click");
    await flushPromises();
    expect(exportTestReportMarkdown).toHaveBeenCalledWith("run-1");
    expect(wrapper.text()).toContain("MD 已保存：C:/reports/example.md");
  });

  it("已导出过的报告直接显示打开 PDF 且不会重复导出", async () => {
    getExistingTestReportPdf.mockResolvedValue("C:/reports/existing.pdf");
    const wrapper = mountDialog();
    await flushPromises();
    const pdfButton = wrapper.find('[data-testid="pdf-action"]');
    expect(getExistingTestReportPdf).toHaveBeenCalledWith("run-1");
    expect(pdfButton.text()).toBe("打开 PDF");
    await pdfButton.trigger("click");
    await flushPromises();
    expect(openTestReportPdf).toHaveBeenCalledWith("C:/reports/existing.pdf");
    expect(exportTestReportPdf).not.toHaveBeenCalled();
  });

  it("把没有结构化字段的旧 Markdown 报告排成可读章节和表格", () => {
    const legacyRun = { ...run, scenarioResults: [], artifacts: [], totalCount: 0, passedCount: 0, failedCount: 0, reportMarkdown: "" };
    const wrapper = mountDialog({ run: legacyRun, fallbackMarkdown: "# 旧测试报告\n\n- 测试结论：不通过\n\n## 汇总\n\n| 用例总数 | 通过 | 失败 |\n| ---: | ---: | ---: |\n| 3 | 2 | 1 |\n\n## 失败详情\n\n### 搜索场景\n\n1. 点击搜索\n2. 核对列表" });
    expect(wrapper.find(".legacy-report-readable").text()).toContain("历史报告兼容视图");
    expect(wrapper.find(".legacy-report-readable").text()).toContain("搜索场景");
    expect(wrapper.find(".legacy-table-wrap table").text()).toContain("3");
    expect(wrapper.find(".report-overview-v2").exists()).toBe(false);
  });
});
