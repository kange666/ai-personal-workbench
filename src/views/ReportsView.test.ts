import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createPinia } from "pinia";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReportRecord } from "../services/backend";

const backend = vi.hoisted(() => ({
  listReports: vi.fn(),
  saveTask: vi.fn(),
  summarizeReportWithAi: vi.fn(),
}));

vi.mock("../services/backend", async (importOriginal) => ({
  ...await importOriginal<typeof import("../services/backend")>(),
  ...backend,
  isTauriRuntime: () => true,
}));

import ReportsView from "./ReportsView.vue";

const originalContent = "# 2026年第36周工作总结\n\n## 项目工作总结\n\n### client\n\n- 新增相关方管理模块\n- 多模块接口增加 deptId 参数";
let wrapper: VueWrapper;

async function renderView(report: ReportRecord) {
  backend.listReports.mockResolvedValue([report]);
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/reports", component: ReportsView }],
  });
  await router.push("/reports");
  await router.isReady();
  wrapper = mount(ReportsView, {
    attachTo: document.body,
    global: { plugins: [createPinia(), router] },
  });
  await flushPromises();
}

beforeEach(() => {
  vi.resetAllMocks();
  backend.saveTask.mockResolvedValue(undefined);
  backend.summarizeReportWithAi.mockResolvedValue(
    "- 相关方管理功能开发完成\n- 修改多模块 deptId 参数逻辑",
  );
});

afterEach(() => wrapper?.unmount());

describe("报告 AI 总结", () => {
  it("把 DeepSeek 总结展示在独立模块中，并保留原报告正文", async () => {
    const report: ReportRecord = {
      id: "report-1",
      reportType: "weekly",
      periodStart: "2026-08-31",
      periodEnd: "2026-09-06",
      title: "2026年第36周工作总结",
      contentMarkdown: originalContent,
      aiSummary: "",
      status: "draft",
      createdAt: "2026-09-04T00:00:00Z",
      updatedAt: "2026-09-04T00:00:00Z",
    };
    await renderView(report);

    expect(wrapper.text()).toContain("原报告内容不会被替换");
    expect(wrapper.text()).toContain("新增相关方管理模块");
    expect(wrapper.text()).not.toContain("AI 润色");

    const summaryButton = wrapper.findAll("button").find(button => button.text().includes("AI 总结"));
    expect(summaryButton).toBeTruthy();
    await summaryButton!.trigger("click");
    await flushPromises();

    expect(backend.summarizeReportWithAi).toHaveBeenCalledWith("report-1");
    expect(wrapper.get(".report-ai-summary").text()).toContain("相关方管理功能开发完成");
    expect(wrapper.get(".report-ai-summary").text()).toContain("修改多模块 deptId 参数逻辑");
    expect(wrapper.text()).toContain("新增相关方管理模块");
    expect(report.contentMarkdown).toBe(originalContent);
  });
});
