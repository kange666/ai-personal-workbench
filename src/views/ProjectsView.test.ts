import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { GitRepositoryStatus } from "../services/backend";

const backend = vi.hoisted(() => ({
  listRepositoryAssets: vi.fn(), listRunningRepositoryProjects: vi.fn(),
  getGitRepositoryStatus: vi.fn(), switchGitRepositoryBranch: vi.fn(),
  getRepositoryAssetDetails: vi.fn(), fetchGitRepository: vi.fn(),
  openRepositoryInEditor: vi.fn(),
}));
vi.mock("../services/backend", async (importOriginal) => ({
  ...await importOriginal<typeof import("../services/backend")>(),
  ...backend, isTauriRuntime: () => true,
}));
import ProjectsView from "./ProjectsView.vue";

const projectPath = "C:/projects/example";
function gitStatus(overrides: Partial<GitRepositoryStatus> = {}): GitRepositoryStatus {
  return {
    repositoryPath: projectPath, currentBranch: "main", branches: ["main", "feature"],
    remoteUrl: "", upstream: "", ahead: 0, behind: 0, userName: "", userEmail: "",
    hasUncommittedChanges: false, mergeInProgress: false, hasWorkbenchStash: false,
    changedFiles: [], credential: { configured: false, username: "", source: "" }, ...overrides,
  };
}
function asset(branch = "main") {
  return {
    path: projectPath, name: "示例项目", isPinned: true, isHidden: false, category: "工具", purpose: "示例",
    defaultBranch: branch, healthLevel: "健康", hasUncommittedChanges: false, changedFileCount: 0,
    aheadCount: 0, behindCount: 0, pendingLevel: "none", conversationCount: 0,
    updatedAt: new Date().toISOString(), lastActivityAt: new Date().toISOString(),
    runtimeStatus: "", runtimeStartedAt: "", lastScannedAt: "", manuallyConfirmed: true,
  };
}
let wrapper: VueWrapper;
async function renderView() {
  const router = createRouter({ history: createMemoryHistory(), routes: [
    { path: "/projects", component: ProjectsView },
    { path: "/project-mapping", component: { template: "<div/>" } },
  ] });
  await router.push("/projects");
  await router.isReady();
  wrapper = mount(ProjectsView, { attachTo: document.body, global: { plugins: [router] } });
  await flushPromises();
  return wrapper;
}
beforeEach(() => {
  vi.resetAllMocks();
  backend.listRepositoryAssets.mockImplementation(async () => [asset()]);
  backend.listRunningRepositoryProjects.mockResolvedValue([]);
  backend.getGitRepositoryStatus.mockImplementation(async () => gitStatus());
  backend.switchGitRepositoryBranch.mockResolvedValue({ message: "已切换到 feature 分支。", output: "", commitHash: "" });
  backend.openRepositoryInEditor.mockResolvedValue(undefined);
});
afterEach(() => { wrapper?.unmount(); });

describe("项目资产列表操作", () => {
  it("移除 CSV，并按启动、Git、打开项目排列；打开编辑器不展开项目详情", async () => {
    await renderView();
    expect(wrapper.text()).not.toContain("CSV");
    expect(wrapper.findAll(".asset-row-actions button").map((button) => button.text())).toEqual(["▶启动", "Git", "打开项目"]);
    await wrapper.get(".open-project").trigger("click");
    await flushPromises();
    expect(backend.openRepositoryInEditor).toHaveBeenCalledWith(projectPath);
    expect(backend.getRepositoryAssetDetails).not.toHaveBeenCalled();
    expect(backend.getGitRepositoryStatus).not.toHaveBeenCalled();
  });

  it("点击分支只读取本地状态，选择后复核并切换、刷新列表", async () => {
    await renderView();
    const chevron = wrapper.get(".branch-picker-trigger svg");
    expect(chevron.attributes("width")).toBe("14");
    expect(chevron.attributes("height")).toBe("14");
    expect(chevron.attributes("aria-hidden")).toBe("true");
    await wrapper.get(".branch-picker-trigger").trigger("click");
    await flushPromises();
    expect(wrapper.get('[role="dialog"]').text()).toContain("工作区干净");
    expect(backend.switchGitRepositoryBranch).not.toHaveBeenCalled();
    expect(backend.fetchGitRepository).not.toHaveBeenCalled();
    expect(backend.getRepositoryAssetDetails).not.toHaveBeenCalled();
    backend.listRepositoryAssets.mockResolvedValue([asset("feature")]);
    await wrapper.get(".branch-option:not(.current)").trigger("click");
    await flushPromises();
    expect(backend.getGitRepositoryStatus).toHaveBeenCalledTimes(2);
    expect(backend.switchGitRepositoryBranch).toHaveBeenCalledWith(projectPath, "feature");
    expect(wrapper.find('[role="dialog"]').exists()).toBe(false);
    expect(wrapper.get(".branch-picker-trigger").text()).toContain("feature");
  });

  it.each(["dirty", "merge"])("以实时状态阻止 %s 工作区切换，不依赖列表缓存", async (kind) => {
    backend.getGitRepositoryStatus.mockResolvedValue(gitStatus({ hasUncommittedChanges: kind === "dirty", mergeInProgress: kind === "merge" }));
    await renderView();
    await wrapper.get(".branch-picker-trigger").trigger("click");
    await flushPromises();
    expect(wrapper.find('[role="alert"]').exists()).toBe(true);
    expect(wrapper.get(".branch-option:not(.current)").attributes("disabled")).toBeDefined();
    expect(backend.switchGitRepositoryBranch).not.toHaveBeenCalled();
  });

  it("弹窗打开后新增修改也会阻止切换", async () => {
    await renderView();
    await wrapper.get(".branch-picker-trigger").trigger("click");
    await flushPromises();
    backend.getGitRepositoryStatus.mockResolvedValue(gitStatus({ hasUncommittedChanges: true }));
    await wrapper.get(".branch-option:not(.current)").trigger("click");
    await flushPromises();
    expect(backend.switchGitRepositoryBranch).not.toHaveBeenCalled();
    expect(wrapper.get('[role="alert"]').text()).toContain("工作区不干净");
  });

  it("切换失败保留弹窗、当前分支并显示错误", async () => {
    backend.switchGitRepositoryBranch.mockRejectedValue(new Error("分支已被其他工作区使用"));
    await renderView();
    await wrapper.get(".branch-picker-trigger").trigger("click");
    await flushPromises();
    await wrapper.get(".branch-option:not(.current)").trigger("click");
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain("分支已被其他工作区使用");
    expect(wrapper.get(".branch-picker-trigger").text()).toContain("main");
  });

  it("编辑器未安装时显示错误，不报告打开成功", async () => {
    backend.openRepositoryInEditor.mockRejectedValue(new Error("未找到 VS Code"));
    await renderView();
    await wrapper.get(".open-project").trigger("click");
    await flushPromises();
    expect(wrapper.get(".scan-message.error").text()).toContain("未找到 VS Code");
  });
});
