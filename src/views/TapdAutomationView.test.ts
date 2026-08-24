import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

const backend = vi.hoisted(() => ({
  executeTapdCodexJob: vi.fn(),
  getTapdStatus: vi.fn(),
  isTauriRuntime: vi.fn(() => true),
  listRepositoryAssets: vi.fn(),
  listTapdCodexJobs: vi.fn(),
  listTapdItems: vi.fn(),
  previewTapdProjectAutomation: vi.fn(),
  saveTapdProjectAutomation: vi.fn(),
  setTapdAutomationPaused: vi.fn(),
  syncTapdItems: vi.fn(),
}));

vi.mock("../services/backend", () => backend);

import TapdAutomationView from "./TapdAutomationView.vue";

const project = {
  workspaceId: "37583308",
  workspaceName: "安全生产管理",
  owner: "测试负责人",
  enabled: true,
  sortOrder: 1,
  repositoryPath: "F:/TB-project/client",
  autoEnabled: true,
  autoExecute: true,
  triggerStatuses: ["new", "reopened"],
  completionStatus: "已解决",
  lastError: "",
  itemCount: 1,
};

const item = {
  itemKey: "37583308:1001",
  id: "1001",
  workspaceId: "37583308",
  itemType: "bug",
  title: "移动端按钮溢出",
  description: "",
  status: "new",
  statusLabel: "待处理",
  priority: "高",
  owner: "测试负责人",
  creator: "",
  iterationId: "",
  beginDate: "",
  dueDate: "2026-08-31",
  createdAt: "2026-08-20T08:00:00Z",
  modifiedAt: "2026-08-21T08:00:00Z",
  sourceUrl: "https://www.tapd.cn/37583308/bugtrace/bugs/view/1001",
  syncedAt: "2026-08-21T08:01:00Z",
};

const job = {
  id: "job-1",
  itemKey: item.itemKey,
  itemId: item.id,
  workspaceId: item.workspaceId,
  repositoryPath: project.repositoryPath,
  status: "queued",
  output: "",
  errorMessage: "",
  baselineHead: "head",
  baselineWorktree: " M src/example.ts",
  resultHead: "",
  changedFiles: [],
  testSummary: "",
  reviewStatus: "pending",
  reviewNote: "",
  triggerSource: "auto",
  sourceModifiedAt: item.modifiedAt,
  triggerReason: "缺陷重新打开",
  executionMode: "manual",
  executionBlockReason: "本地项目已有未提交修改，已自动降级为手工执行。",
  testRequired: true,
  processReportPath: "F:/reports/job-1.md",
  createdAt: "2026-08-21T08:02:00Z",
  updatedAt: "2026-08-21T08:02:00Z",
};

function mountView() {
  return mount(TapdAutomationView, {
    global: {
      stubs: {
        RouterLink: { template: "<a><slot /></a>" },
      },
    },
  });
}

describe("TapdAutomationView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    backend.isTauriRuntime.mockReturnValue(true);
    backend.getTapdStatus.mockResolvedValue({
      configured: true,
      source: "Windows 凭据库",
      authMode: "token",
      workspaceId: project.workspaceId,
      workspaceName: project.workspaceName,
      owner: project.owner,
      itemCount: 1,
      warnings: [],
      autoFixEnabled: true,
      autoFixRepositoryPath: project.repositoryPath,
      automationPaused: false,
      projects: [project],
    });
    backend.listTapdItems.mockResolvedValue([item]);
    backend.listTapdCodexJobs.mockResolvedValue([job]);
    backend.listRepositoryAssets.mockResolvedValue([
      { path: project.repositoryPath, name: "client" },
    ]);
    backend.previewTapdProjectAutomation.mockResolvedValue({
      workspaceId: project.workspaceId,
      totalItems: 1,
      matchedCount: 1,
      pendingCount: 1,
      items: [
        {
          itemKey: item.itemKey,
          itemId: item.id,
          title: item.title,
          statusLabel: item.statusLabel,
          priority: item.priority,
          dueDate: item.dueDate,
          triggerReason: "当前为待处理状态",
        },
      ],
    });
    backend.setTapdAutomationPaused.mockResolvedValue(true);
  });

  it("主界面只显示队列决策信息，规则配置默认收进侧边抽屉", async () => {
    const wrapper = mountView();
    await flushPromises();

    expect(wrapper.find(".automation-rule-drawer").exists()).toBe(false);
    expect(wrapper.find(".queue-row").text()).toContain("高");
    expect(wrapper.find(".queue-row").text()).toContain("缺陷重新打开");
    expect(wrapper.find(".queue-row").text()).toContain("手工开始");
    expect(wrapper.find(".queue-row").text()).toContain("未提交修改");

    const configureButton = wrapper.findAll("button").find((button) => button.text() === "配置规则");
    expect(configureButton).toBeTruthy();
    await configureButton!.trigger("click");
    expect(wrapper.find(".automation-rule-drawer").exists()).toBe(true);
    expect(wrapper.find(".automation-rule-drawer").text()).toContain("人工确认后流转到");

    const previewButton = wrapper.findAll("button").find((button) => button.text() === "预览命中");
    await previewButton!.trigger("click");
    await flushPromises();
    expect(backend.previewTapdProjectAutomation).toHaveBeenCalledOnce();
    expect(wrapper.find(".rule-preview").text()).toContain("下次同步预计新增 1 个队列任务");
    wrapper.unmount();
  });

  it("可以从主界面全局暂停自动处理", async () => {
    const wrapper = mountView();
    await flushPromises();
    const pauseButton = wrapper.findAll("button").find((button) => button.text() === "暂停自动处理");
    await pauseButton!.trigger("click");
    await flushPromises();
    expect(backend.setTapdAutomationPaused).toHaveBeenCalledWith(true);
    expect(wrapper.find(".automation-paused").text()).toContain("自动处理已暂停");
    wrapper.unmount();
  });
});
