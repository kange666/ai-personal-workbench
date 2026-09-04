import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { GitRepositoryStatus } from "../services/backend";

const backend = vi.hoisted(() => ({
  listRepositoryAssets: vi.fn(), listRunningRepositoryProjects: vi.fn(),
  getGitRepositoryStatus: vi.fn(), switchGitRepositoryBranch: vi.fn(),
  getRepositoryAssetDetails: vi.fn(), fetchGitRepository: vi.fn(),
  getGitRepositoryFileDiff: vi.fn(), discardGitRepositoryChanges: vi.fn(),
  getGitRepositoryCommitFiles: vi.fn(), getGitRepositoryCommitFileDiff: vi.fn(),
  stageGitRepositoryChanges: vi.fn(), smartSyncGitRepository: vi.fn(),
  generateCommitPlan: vi.fn(), commitGitPlanGroup: vi.fn(), pushGitRepository: vi.fn(),
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
  backend.getRepositoryAssetDetails.mockResolvedValue({ conversations: [], commits: [], associations: [], nextAction: "" });
  backend.fetchGitRepository.mockResolvedValue({ message: "远程状态已更新。", output: "", commitHash: "" });
  backend.discardGitRepositoryChanges.mockResolvedValue({ message: "已放弃 1 个选中文件的本地修改。", output: "", commitHash: "" });
  backend.getGitRepositoryCommitFiles.mockResolvedValue([]);
  backend.getGitRepositoryCommitFileDiff.mockResolvedValue({ path: "", diff: "", truncated: false, additions: 0, modifications: 0, deletions: 0 });
  backend.stageGitRepositoryChanges.mockResolvedValue({ message: "文件已暂存。", output: "", commitHash: "" });
  backend.smartSyncGitRepository.mockResolvedValue({ message: "远程已同步。", output: "", conflictsResolved: false });
  backend.generateCommitPlan.mockResolvedValue({
    id: "plan", repositoryPath: projectPath, status: "suggested", riskLevel: "低", summary: "提交所选修改",
    groupingMode: "single", generator: "rules", model: "", generationWarning: "", excludedFiles: [], createdAt: new Date().toISOString(),
    groups: [{ id: "group", title: "项目修改", commitMessage: "feat(project): 更新项目", files: ["src/a.ts"], riskNotes: "", verificationNotes: "", status: "suggested" }],
  });
  backend.commitGitPlanGroup.mockResolvedValue({ message: "提交完成。", output: "", commitHash: "abcdef123456" });
  backend.pushGitRepository.mockResolvedValue({ message: "推送完成。", output: "", commitHash: "abcdef123456" });
  backend.openRepositoryInEditor.mockResolvedValue(undefined);
});
afterEach(() => {
  wrapper?.unmount();
  vi.useRealTimers();
});

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
    expect(wrapper.get('[role="dialog"]').text()).toContain("当前分支");
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

  it("操作成功或失败提示显示 3 秒后自动关闭", async () => {
    vi.useFakeTimers();
    await renderView();

    await wrapper.get(".open-project").trigger("click");
    await flushPromises();
    expect(wrapper.get(".scan-message").text()).toContain("已请求使用 VS Code 打开");
    vi.advanceTimersByTime(2999);
    await wrapper.vm.$nextTick();
    expect(wrapper.find(".scan-message").exists()).toBe(true);
    vi.advanceTimersByTime(1);
    await wrapper.vm.$nextTick();
    expect(wrapper.find(".scan-message").exists()).toBe(false);

    backend.openRepositoryInEditor.mockRejectedValueOnce(new Error("未找到 VS Code"));
    await wrapper.get(".open-project").trigger("click");
    await flushPromises();
    expect(wrapper.get(".scan-message.error").text()).toContain("未找到 VS Code");
    vi.advanceTimersByTime(3000);
    await wrapper.vm.$nextTick();
    expect(wrapper.find(".scan-message").exists()).toBe(false);
  });

  it("项目抽屉不显示英文标识，并按指定顺序单行展示头部操作", async () => {
    await renderView();
    await wrapper.findAll(".asset-row-actions button")[1].trigger("click");
    await flushPromises();
    const drawer = wrapper.get(".asset-drawer");
    expect(drawer.text()).not.toContain("PROJECT ASSET");
    expect(drawer.findAll(".drawer-header-actions button").map((button) => button.text())).toEqual([
      "▶ 运行", "接口文档", "编辑", "★ 已置顶", "隐藏", "×",
    ]);
  });

  it("提交历史显示提交人，并按提交和文件两级展开差异", async () => {
    const commitHash = "abcdef1234567890abcdef1234567890abcdef12";
    backend.getRepositoryAssetDetails.mockResolvedValue({
      conversations: [], associations: [], nextAction: "",
      commits: [{ hash: commitHash, subject: "feat(project): 更新功能", committedAt: "2026-09-04T10:00:00+08:00", authorName: "张三", authorEmail: "zhangsan@example.com" }],
    });
    backend.getGitRepositoryCommitFiles.mockResolvedValue([
      { path: "src/history.ts", status: "M", label: "修改" },
    ]);
    backend.getGitRepositoryCommitFileDiff.mockResolvedValue({
      path: "src/history.ts", diff: "-old\n+new", truncated: false,
      additions: 0, modifications: 1, deletions: 0,
    });
    await renderView();
    await wrapper.findAll(".asset-row-actions button")[1].trigger("click");
    await flushPromises();
    expect(wrapper.get(".git-history").text()).toContain("提交人：张三");

    await wrapper.get(".git-history-toggle").trigger("click");
    await flushPromises();
    expect(backend.getGitRepositoryCommitFiles).toHaveBeenCalledWith(projectPath, commitHash);
    expect(wrapper.get(".git-commit-file-row").text()).toContain("src/history.ts");

    await wrapper.get(".git-commit-file-row").trigger("click");
    await flushPromises();
    expect(backend.getGitRepositoryCommitFileDiff).toHaveBeenCalledWith(projectPath, commitHash, "src/history.ts");
    expect(wrapper.get(".history-diff").text()).toContain("修改 1");
    expect(wrapper.get(".history-diff").text()).toContain("-old");
    expect(wrapper.get(".history-diff").text()).toContain("+new");
  });

  it("在每个文件下方独立展开多个差异并显示增删改统计", async () => {
    backend.getGitRepositoryStatus.mockResolvedValue(gitStatus({
      remoteUrl: "https://example.com/repo.git", upstream: "origin/main",
      hasUncommittedChanges: true,
      changedFiles: [
        { path: "src/a.ts", indexStatus: " ", worktreeStatus: "M", label: "已修改" },
        { path: "src/b.ts", indexStatus: " ", worktreeStatus: "M", label: "已修改" },
      ],
    }));
    backend.getGitRepositoryFileDiff.mockImplementation(async (_path, file) => ({
      path: file, stagedDiff: "", unstagedDiff: "+new\n-old", truncated: false,
      additions: file.endsWith("a.ts") ? 23 : 2, modifications: 3, deletions: 3,
    }));
    await renderView();
    await wrapper.findAll(".asset-row-actions button")[1].trigger("click");
    await flushPromises();
    const diffButtons = wrapper.findAll(".git-file-actions button").filter((button) => button.text() === "查看差异");
    await diffButtons[0].trigger("click");
    await diffButtons[1].trigger("click");
    await flushPromises();
    expect(wrapper.findAll(".git-diff-preview")).toHaveLength(2);
    expect(wrapper.findAll(".git-changed-file-entry")[0].text()).toContain("新增 23");
    expect(wrapper.findAll(".git-changed-file-entry")[0].text()).toContain("修改 3");
    expect(wrapper.findAll(".git-changed-file-entry")[0].text()).toContain("删除 3");
  });

  it("未选择文件时禁用还原和智能提交，选择后在抽屉内确认", async () => {
    backend.getGitRepositoryStatus.mockResolvedValue(gitStatus({
      remoteUrl: "https://example.com/repo.git", upstream: "origin/main",
      hasUncommittedChanges: true,
      changedFiles: [{ path: "src/a.ts", indexStatus: " ", worktreeStatus: "M", label: "已修改" }],
    }));
    await renderView();
    await wrapper.findAll(".asset-row-actions button")[1].trigger("click");
    await flushPromises();
    const action = (label: string) => wrapper.findAll(".git-remote-actions button").find((button) => button.text().includes(label))!;
    expect(action("还原").attributes("disabled")).toBeDefined();
    expect(action("智能提交").attributes("disabled")).toBeDefined();
    await wrapper.get('.git-changed-file-entry input[type="checkbox"]').setValue(true);
    expect(action("还原").attributes("disabled")).toBeUndefined();
    expect(action("智能提交").attributes("disabled")).toBeUndefined();
    await action("还原").trigger("click");
    expect(wrapper.get(".discard-confirmation").text()).toContain("操作无法撤销");
    await wrapper.findAll(".discard-confirmation button")[1].trigger("click");
    await flushPromises();
    expect(wrapper.get(".drawer-git-feedback").text()).toContain("已放弃 1 个选中文件");
    expect(backend.discardGitRepositoryChanges).toHaveBeenCalledWith(projectPath, ["src/a.ts"]);
  });

  it("智能提交按暂存、更新、同步、生成信息、提交和推送的顺序执行", async () => {
    const dirtyStatus = gitStatus({
      remoteUrl: "https://example.com/repo.git", upstream: "origin/main",
      hasUncommittedChanges: true,
      changedFiles: [{ path: "src/a.ts", indexStatus: " ", worktreeStatus: "M", label: "已修改" }],
    });
    backend.getGitRepositoryStatus.mockResolvedValue(dirtyStatus);
    await renderView();
    await wrapper.findAll(".asset-row-actions button")[1].trigger("click");
    await flushPromises();
    vi.clearAllMocks();
    backend.getGitRepositoryStatus.mockResolvedValue(gitStatus());
    backend.listRepositoryAssets.mockResolvedValue([asset()]);
    backend.getRepositoryAssetDetails.mockResolvedValue({ conversations: [], commits: [], associations: [], nextAction: "" });
    await wrapper.get('.git-changed-file-entry input[type="checkbox"]').setValue(true);
    const smartButton = wrapper.findAll(".git-remote-actions button").find((button) => button.text() === "智能提交")!;
    await smartButton.trigger("click");
    await flushPromises();

    const order = [
      backend.stageGitRepositoryChanges,
      backend.fetchGitRepository,
      backend.smartSyncGitRepository,
      backend.generateCommitPlan,
      backend.commitGitPlanGroup,
      backend.pushGitRepository,
    ].map((mock) => mock.mock.invocationCallOrder[0]);
    expect(order).toEqual([...order].sort((left, right) => left - right));
    expect(wrapper.get(".smart-commit-progress").classes()).toContain("completed");
    expect(wrapper.get(".smart-commit-progress").text()).toContain("100%");
    expect(wrapper.get(".drawer-git-feedback").text()).toContain("智能提交完成");
  });
});
