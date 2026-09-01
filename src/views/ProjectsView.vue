<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  abortGitRepositoryMerge,
  clearGitDefaultCredential,
  commitGitPlanGroup,
  fetchGitRepository,
  generateCommitPlan,
  getGitScanConfiguration,
  getGitScanStatus,
  getGitRepositoryStatus,
  getGitRepositoryFileDiff,
  getRepositoryAssetDetails,
  isTauriRuntime,
  listRepositoryAssets,
  listRunningRepositoryProjects,
  mergeGitRepositoryBranch,
  openRepositoryRuntimeUrl,
  pullGitRepository,
  pushGitRepository,
  resolveGitPullConflicts,
  restoreGitRepositoryStash,
  revertGitRepositoryCommit,
  saveGitDefaultCredential,
  saveGitScanConfiguration,
  saveRepositoryAsset,
  scanGitRepositories,
  setRepositoryCategory,
  setRepositoryHidden,
  setRepositoryPinned,
  startRepositoryProject,
  stageGitRepositoryChanges,
  stashGitRepositoryChanges,
  stopRepositoryProject,
  switchGitRepositoryBranch,
  unstageGitRepositoryChanges,
  type GitOperationResult,
  type CommitGroupingMode,
  type GitChangedFile,
  type GitFileDiff,
  type GitPullConflict,
  type GitRepositoryStatus,
  type GitScanConfiguration,
  type RepositoryAsset,
  type RepositoryAssetDetails,
  type RepositoryAssetUpdate,
  type RunningProjectProcess,
} from "../services/backend";

interface GitOperationLogEntry {
  id: number;
  action: string;
  status: "success" | "error";
  message: string;
  output: string;
  occurredAt: string;
}

const router = useRouter();
const route = useRoute();
const assets = ref<RepositoryAsset[]>([]);
const selected = ref<RepositoryAsset | null>(null);
const details = ref<RepositoryAssetDetails>({ conversations: [], commits: [], associations: [], nextAction: "" });
const gitStatus = ref<GitRepositoryStatus | null>(null);
const showHiddenList = ref(false);
const activeTab = ref<"overview" | "relations" | "git">("overview");
const viewMode = ref<"recent" | "active" | "attention" | "stale">("recent");
const query = ref("");
const categoryFilter = ref("全部分类");
const health = ref("全部状态");
const changes = ref("全部工作区");
const loading = ref(false);
const editing = ref(false);
const message = ref("");
const error = ref("");
const gitOperationLogs = ref<GitOperationLogEntry[]>([]);
const fileDiff = ref<GitFileDiff | null>(null);
const diffLoadingPath = ref("");
const postCommitPrompt = ref<{ commitHash: string; message: string } | null>(null);
const switchBranch = ref("");
const mergeBranch = ref("");
const credentialUsername = ref("lzsk");
const credentialSecret = ref("");
const categoryEditor = ref<RepositoryAsset | null>(null);
const categoryDraft = ref("");
const categorySaving = ref(false);
const startingPath = ref("");
const runningPaths = ref<string[]>([]);
const runningProcesses = ref<RunningProjectProcess[]>([]);
const showScanSettings = ref(false);
const scanConfiguration = reactive<GitScanConfiguration>({ roots: [], maxDepth: 3, excludedNames: [] });
const newScanRoot = ref("");
const newExcludedName = ref("");
const scanSaving = ref(false);
const lastScanErrors = ref<string[]>([]);
const lastScanAt = ref("");
const nowTick = ref(Date.now());
let runtimeTimer: number | undefined;
const commitMessages = reactive<Record<string, string>>({});
const selectedStageFiles = ref<string[]>([]);
const autoFetchingRemote = ref(false);
const remoteRefreshNote = ref("");
const pullConflict = ref<GitPullConflict | null>(null);
const resolvingConflict = ref(false);
const commitGroupingMode = ref<CommitGroupingMode>("single");
const form = reactive<RepositoryAssetUpdate>({
  path: "", category: "待确认", purpose: "", technologyStack: "", mainModules: "",
  installCommand: "", startCommand: "", testCommand: "", buildCommand: "", commandSource: "",
});

const visibleAssets = computed(() => assets.value.filter((item) => !item.isHidden));
const hiddenAssets = computed(() => assets.value.filter((item) => item.isHidden));
const categories = computed(() => Array.from(new Set(visibleAssets.value.map((item) => item.category.trim() || "待确认"))).sort((left, right) => left === "待确认" ? 1 : right === "待确认" ? -1 : left.localeCompare(right, "zh-CN")));
const runtimeMap = computed(() => new Map(runningProcesses.value.map((item) => [item.projectPath, item])));
const stageableFiles = computed(() => gitStatus.value?.changedFiles.filter(hasUnstagedFile) ?? []);
const allStageableFilesSelected = computed(() => stageableFiles.value.length > 0 && stageableFiles.value.every((file) => selectedStageFiles.value.includes(file.path)));
const canPush = computed(() => {
  if (!gitStatus.value?.remoteUrl) return false;
  return gitStatus.value.upstream ? gitStatus.value.ahead > 0 : details.value.commits.length > 0;
});
const associationGroups = computed(() => {
  const labels: Record<string, string> = { codex: "Codex 任务", tapd: "TAPD 缺陷", test: "测试记录", work: "工作记录", report: "报告", deployment: "部署与发布", docs: "项目文档", build: "构建配置", remote: "远程仓库", runtime: "本地运行地址" };
  const groups = new Map<string, RepositoryAssetDetails["associations"]>();
  for (const item of details.value.associations) groups.set(item.kind, [...(groups.get(item.kind) ?? []), item]);
  return [...groups.entries()].map(([kind, items]) => ({ kind, label: labels[kind] || kind, items }));
});
function activityDays(item: RepositoryAsset) {
  const time = Date.parse(item.lastActivityAt || item.updatedAt);
  return Number.isFinite(time) ? Math.floor((nowTick.value - time) / 86400000) : 9999;
}
function isRuntimeActive(item: RepositoryAsset) { return runtimeMap.value.has(item.path); }
function smartScore(item: RepositoryAsset) {
  return Number(item.isPinned) * 10000
    + Number(isRuntimeActive(item)) * 1000
    + Number(item.conversationCount > 0 && activityDays(item) <= 7) * 700
    + Number(item.hasUncommittedChanges) * 500
    + Number(item.behindCount > 0 || item.runtimeStatus === "failed" || item.healthLevel === "失败") * 300;
}
const filtered = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  return visibleAssets.value.filter((item) => {
    if (health.value !== "全部状态" && item.healthLevel !== health.value) return false;
    if (categoryFilter.value !== "全部分类" && item.category !== categoryFilter.value) return false;
    if (changes.value === "有未提交修改" && !item.hasUncommittedChanges) return false;
    if (changes.value === "工作区干净" && item.hasUncommittedChanges) return false;
    if (viewMode.value === "recent" && activityDays(item) >= 60) return false;
    if (viewMode.value === "active" && !isRuntimeActive(item) && !item.hasUncommittedChanges && activityDays(item) > 14) return false;
    if (viewMode.value === "attention" && item.pendingLevel === "none") return false;
    if (viewMode.value === "stale" && activityDays(item) < 60) return false;
    return !keyword || [item.name, item.path, item.purpose, item.defaultBranch, item.category].some((value) => value.toLowerCase().includes(keyword));
  }).sort((left, right) => smartScore(right) - smartScore(left) || Date.parse(right.lastActivityAt) - Date.parse(left.lastActivityAt) || left.name.localeCompare(right.name, "zh-CN"));
});
const dirtyCount = computed(() => visibleAssets.value.filter((item) => item.hasUncommittedChanges).length);
const failedCount = computed(() => visibleAssets.value.filter((item) => item.healthLevel === "失败").length);
const attentionCount = computed(() => visibleAssets.value.filter((item) => item.pendingLevel !== "none").length);
const latestScanAt = computed(() => lastScanAt.value || visibleAssets.value.map((item) => item.lastScannedAt).filter(Boolean).sort().at(-1) || "");
const selectedRuntime = computed<RunningProjectProcess | null>(() => {
  if (!selected.value) return null;
  return runtimeMap.value.get(selected.value.path) ?? (selected.value.runtimeStartedAt ? {
    projectPath: selected.value.path,
    projectName: selected.value.name,
    command: selected.value.startCommand,
    processId: 0,
    status: selected.value.runtimeStatus || "stopped",
    startedAt: selected.value.runtimeStartedAt,
    localUrl: selected.value.runtimeLocalUrl,
    logPath: selected.value.runtimeLogPath,
    logExcerpt: selected.value.runtimeLogExcerpt,
    errorMessage: selected.value.runtimeError,
  } : null);
});
const mergeOptions = computed(() => gitStatus.value?.branches.filter((branch) => branch !== gitStatus.value?.currentBranch) ?? []);

function healthClass(value: string) { return value === "健康" ? "healthy" : value === "警告" ? "warning" : value === "失败" ? "failed" : "unknown"; }
function hasStagedFile(file: GitChangedFile) { return file.indexStatus !== " " && file.indexStatus !== "?"; }
function hasUnstagedFile(file: GitChangedFile) { return file.worktreeStatus !== " "; }
function stageFileLabel(file: GitChangedFile) { return hasStagedFile(file) ? (hasUnstagedFile(file) ? "部分已暂存" : "已暂存") : "未暂存"; }
function toggleAllStageableFiles(event: Event) { selectedStageFiles.value = (event.target as HTMLInputElement).checked ? stageableFiles.value.map((file) => file.path) : []; }
function branchSyncSummary(item: RepositoryAsset) {
  if (item.aheadCount > 0 && item.behindCount > 0) return `领先 ${item.aheadCount} · 落后 ${item.behindCount}`;
  if (item.aheadCount > 0) return `领先 ${item.aheadCount}`;
  if (item.behindCount > 0) return `落后 ${item.behindCount}`;
  return "";
}
function diffLineClass(line: string) {
  if (line.startsWith("+++") || line.startsWith("---") || line.startsWith("diff ") || line.startsWith("index ")) return "meta";
  if (line.startsWith("+")) return "addition";
  if (line.startsWith("-")) return "deletion";
  if (line.startsWith("@@")) return "hunk";
  return "context";
}
function runtimeLabel(value?: string) { return value === "starting" ? "启动中" : value === "running" ? "运行正常" : value === "failed" ? "编译失败" : value === "stopped" ? "已停止" : "未启动"; }
function runtimeClass(value?: string) { return value || "idle"; }
function formatTime(value?: string) { return value ? new Date(value).toLocaleString("zh-CN", { hour12: false }) : "暂无记录"; }
function runtimeDuration(value?: string) {
  if (!value) return "—";
  const seconds = Math.max(0, Math.floor((nowTick.value - Date.parse(value)) / 1000));
  if (seconds < 60) return `${seconds} 秒`;
  const minutes = Math.floor(seconds / 60);
  return minutes < 60 ? `${minutes} 分钟` : `${Math.floor(minutes / 60)} 小时 ${minutes % 60} 分`;
}
function needsProjectConfirmation(item: RepositoryAsset) { return !item.manuallyConfirmed && (!item.category.trim() || item.category === "待确认"); }
function projectDescription(item: RepositoryAsset) { return item.purpose || (needsProjectConfirmation(item) ? "AI 推断待确认" : "已完成分类"); }
function isProjectRunning(path: string) { return runtimeMap.value.has(path) || runningPaths.value.includes(path); }
function projectRuntimeUrl(item: RepositoryAsset) {
  const current = runtimeMap.value.get(item.path);
  if (current) return current.localUrl;
  return item.runtimeStatus === "running" || item.runtimeStatus === "starting" ? item.runtimeLocalUrl : "";
}
function fillForm(item: RepositoryAsset) { Object.assign(form, { path: item.path, category: item.category, purpose: item.purpose, technologyStack: item.technologyStack, mainModules: item.mainModules, installCommand: item.installCommand, startCommand: item.startCommand, testCommand: item.testCommand, buildCommand: item.buildCommand, commandSource: item.commandSource }); }
function syncCommitMessages() { for (const key of Object.keys(commitMessages)) delete commitMessages[key]; for (const group of details.value.commitPlan?.groups ?? []) commitMessages[group.id] = group.commitMessage; }
function discardStaleCommitPlan() {
  const plan = details.value.commitPlan;
  if (!plan || !gitStatus.value) return;
  const normalize = (path: string) => path.replaceAll("\\", "/");
  const current = new Set(gitStatus.value.changedFiles.map((file) => normalize(file.path)));
  const planned = new Set([
    ...plan.groups.filter((group) => group.status !== "committed").flatMap((group) => group.files.map(normalize)),
    ...plan.excludedFiles.map(normalize),
  ]);
  if (current.size !== planned.size || [...current].some((path) => !planned.has(path))) details.value.commitPlan = undefined;
}

async function load() {
  if (!isTauriRuntime()) return;
  loading.value = true; error.value = "";
  try {
    const runningProjects = await listRunningRepositoryProjects();
    const repositoryAssets = await listRepositoryAssets();
    runningProcesses.value = runningProjects;
    runningPaths.value = runningProjects.map((item) => item.projectPath);
    assets.value = repositoryAssets;
  } catch (cause) { error.value = String(cause); } finally { loading.value = false; }
}
async function refreshRuntime() {
  if (!isTauriRuntime()) return;
  try {
    const runningProjects = await listRunningRepositoryProjects();
    runningProcesses.value = runningProjects;
    runningPaths.value = runningProjects.map((item) => item.projectPath);
    assets.value = await listRepositoryAssets();
    if (selected.value) selected.value = assets.value.find((item) => item.path === selected.value?.path) ?? selected.value;
  } catch { /* 轮询失败不覆盖用户正在查看的操作消息。 */ }
}
async function loadGitStatus(path: string) {
  gitStatus.value = await getGitRepositoryStatus(path);
  selectedStageFiles.value = [];
  if (fileDiff.value && !gitStatus.value.changedFiles.some((file) => file.path === fileDiff.value?.path)) fileDiff.value = null;
  switchBranch.value = gitStatus.value.currentBranch;
  mergeBranch.value = gitStatus.value.branches.find((branch) => branch !== gitStatus.value?.currentBranch) ?? "";
  credentialUsername.value = gitStatus.value.credential.username || "lzsk";
}
async function autoFetchRemoteStatus(path: string) {
  if (!gitStatus.value?.remoteUrl || autoFetchingRemote.value) return;
  autoFetchingRemote.value = true;
  remoteRefreshNote.value = "正在自动更新远程状态…";
  try {
    const result = await fetchGitRepository(path);
    recordGitOperation("自动更新远程", "success", result.message, result.output);
    await loadGitStatus(path);
    remoteRefreshNote.value = gitStatus.value?.behind
      ? `发现 ${gitStatus.value.behind} 个远程新提交，可以拉取。`
      : "远程状态已更新，当前没有新代码。";
  } catch (cause) {
    remoteRefreshNote.value = `自动更新远程失败：${String(cause)}`;
    recordGitOperation("自动更新远程", "error", "自动更新远程失败", String(cause));
  } finally {
    autoFetchingRemote.value = false;
  }
}
async function openGitOperations() {
  activeTab.value = "git";
  if (selected.value) await autoFetchRemoteStatus(selected.value.path);
}
async function openAsset(item: RepositoryAsset, tab: "overview" | "relations" | "git" = "overview", refreshRemote = true) {
  if (selected.value?.path !== item.path) {
    gitOperationLogs.value = [];
    fileDiff.value = null;
    postCommitPrompt.value = null;
  }
  selected.value = item; activeTab.value = tab; editing.value = false; fillForm(item); error.value = ""; remoteRefreshNote.value = "";
  try {
    [details.value] = await Promise.all([getRepositoryAssetDetails(item.path), loadGitStatus(item.path)]);
    discardStaleCommitPlan();
    syncCommitMessages();
    if (tab === "git" && refreshRemote) await autoFetchRemoteStatus(item.path);
  } catch (cause) { error.value = String(cause); details.value = { conversations: [], commits: [], associations: [], nextAction: "" }; gitStatus.value = null; }
}
async function refreshSelected() {
  const path = selected.value?.path; await load(); if (!path) return;
  const refreshed = assets.value.find((item) => item.path === path); if (refreshed) await openAsset(refreshed, activeTab.value, false);
}
async function scan() {
  loading.value = true; error.value = ""; message.value = "";
  try { const result = await scanGitRepositories(); lastScanAt.value = new Date().toISOString(); lastScanErrors.value = result.errorDetails; message.value = result.errors ? `已扫描 ${result.repositoriesFound} 个仓库，发现 ${result.errors} 个读取错误。` : `已扫描 ${result.repositoriesFound} 个仓库，Git 健康检查全部可读取。`; await load(); }
  catch (cause) { error.value = String(cause); } finally { loading.value = false; }
}
async function togglePin(item: RepositoryAsset) {
  error.value = "";
  try { await setRepositoryPinned(item.path, !item.isPinned); message.value = item.isPinned ? `已取消置顶 ${item.name}。` : `已置顶 ${item.name}。`; await refreshSelected(); }
  catch (cause) { error.value = String(cause); }
}
async function toggleHidden(item: RepositoryAsset, hidden: boolean) {
  error.value = "";
  try {
    await setRepositoryHidden(item.path, hidden);
    message.value = hidden ? `已隐藏 ${item.name}，可在隐藏列表中恢复。` : `已恢复显示 ${item.name}。`;
    if (hidden && selected.value?.path === item.path) selected.value = null;
    await load();
  } catch (cause) { error.value = String(cause); }
}
function openCategoryEditor(item: RepositoryAsset) {
  categoryEditor.value = item;
  categoryDraft.value = item.category === "待确认" ? "" : item.category;
}
async function saveQuickCategory() {
  if (!categoryEditor.value || !categoryDraft.value.trim()) return;
  categorySaving.value = true; error.value = "";
  try {
    const item = categoryEditor.value;
    const previousCategory = item.category;
    const category = categoryDraft.value.trim();
    await setRepositoryCategory(item.path, category);
    if (categoryFilter.value === previousCategory) categoryFilter.value = category;
    message.value = `${item.name} 已归类为“${category}”。`;
    categoryEditor.value = null;
    await load();
  } catch (cause) { error.value = String(cause); }
  finally { categorySaving.value = false; }
}
async function toggleProjectProcess(item: RepositoryAsset) {
  if (startingPath.value) return;
  startingPath.value = item.path; error.value = ""; message.value = "";
  try {
    const result = isProjectRunning(item.path)
      ? await stopRepositoryProject(item.path)
      : await startRepositoryProject(item.path);
    message.value = result.managed ? `${result.message} 启动命令：${result.command}` : result.message;
    await refreshRuntime();
  } catch (cause) { error.value = String(cause); }
  finally { startingPath.value = ""; }
}
async function openProjectFromRoute() {
  const projectPath = typeof route.query.project === "string" ? route.query.project.trim().toLocaleLowerCase() : "";
  if (!projectPath || !assets.value.length) return;
  const item = assets.value.find(asset => asset.path.trim().toLocaleLowerCase() === projectPath);
  if (item) await openAsset(item, route.query.tab === "git" ? "git" : "overview");
}
watch(() => `${String(route.query.project || "")}|${String(route.query.tab || "")}`, () => { void openProjectFromRoute(); });
async function openScanConfiguration() {
  error.value = "";
  try {
    const [configuration, status] = await Promise.all([getGitScanConfiguration(), getGitScanStatus()]);
    Object.assign(scanConfiguration, configuration);
    lastScanAt.value = status.lastScannedAt;
    lastScanErrors.value = status.errors;
    showScanSettings.value = true;
  } catch (cause) { error.value = String(cause); }
}
function addScanRoot() {
  const value = newScanRoot.value.trim();
  if (value && !scanConfiguration.roots.some((root) => root.toLowerCase() === value.toLowerCase())) scanConfiguration.roots.push(value);
  newScanRoot.value = "";
}
function addExcludedName() {
  const value = newExcludedName.value.trim();
  if (value && !value.includes("/") && !value.includes("\\") && !scanConfiguration.excludedNames.some((name) => name.toLowerCase() === value.toLowerCase())) scanConfiguration.excludedNames.push(value);
  newExcludedName.value = "";
}
async function persistScanConfiguration(runAfterSave = false) {
  if (!scanConfiguration.roots.length) { error.value = "至少保留一个扫描根目录。"; return; }
  scanSaving.value = true; error.value = "";
  try {
    await saveGitScanConfiguration({ roots: [...scanConfiguration.roots], maxDepth: scanConfiguration.maxDepth, excludedNames: [...scanConfiguration.excludedNames] });
    showScanSettings.value = false;
    message.value = "扫描范围已保存。";
    if (runAfterSave) await scan();
  } catch (cause) { error.value = String(cause); }
  finally { scanSaving.value = false; }
}
function continueWork() {
  const conversation = details.value.conversations[0];
  if (conversation) router.push({ path: "/tokens", query: { conversation: conversation.id } });
  else router.push({ path: "/work-records", query: { project: selected.value?.name || "" } });
}
function openAssociation(route: string) { if (/^https?:\/\//i.test(route)) window.open(route, "_blank", "noopener,noreferrer"); else if (route) router.push(route); }
async function copyRuntimeLog() {
  const log = selectedRuntime.value?.logExcerpt || selectedRuntime.value?.errorMessage || "暂无运行日志";
  await navigator.clipboard.writeText(log);
  message.value = "运行日志已复制。";
}
async function openRuntimeUrl(url: string) {
  if (!url) return;
  error.value = "";
  try {
    if (isTauriRuntime()) await openRepositoryRuntimeUrl(url);
    else window.open(url, "_blank", "noopener,noreferrer");
  } catch (cause) { error.value = String(cause); }
}
async function planCommit() {
  if (!selected.value) return; loading.value = true; error.value = "";
  try { details.value.commitPlan = await generateCommitPlan(selected.value.path, commitGroupingMode.value); syncCommitMessages(); message.value = "提交建议已生成，Git 暂存区没有变化。"; }
  catch (cause) { error.value = String(cause); } finally { loading.value = false; }
}
function recordGitOperation(action: string, status: "success" | "error", resultMessage: string, output = "") {
  gitOperationLogs.value.unshift({
    id: Date.now() + Math.floor(Math.random() * 1000),
    action,
    status,
    message: resultMessage,
    output,
    occurredAt: new Date().toISOString(),
  });
  gitOperationLogs.value = gitOperationLogs.value.slice(0, 20);
}
async function runGitAction(actionName: string, action: () => Promise<GitOperationResult>): Promise<GitOperationResult | null> {
  loading.value = true; error.value = ""; message.value = "";
  try {
    const result = await action();
    message.value = result.message;
    recordGitOperation(actionName, "success", result.message, result.output);
    await refreshSelected();
    return result;
  } catch (cause) {
    error.value = String(cause);
    recordGitOperation(actionName, "error", "操作失败", String(cause));
    return null;
  } finally { loading.value = false; }
}
async function pullRepository() {
  if (!selected.value) return;
  const path = selected.value.path;
  loading.value = true; error.value = ""; message.value = "";
  try {
    const result = await pullGitRepository(path);
    message.value = result.message;
    recordGitOperation("拉取代码", result.conflict ? "error" : "success", result.message, result.output);
    if (result.conflict) {
      pullConflict.value = result.conflict;
      await loadGitStatus(path);
    } else {
      pullConflict.value = null;
      await refreshSelected();
    }
  } catch (cause) { error.value = String(cause); recordGitOperation("拉取代码", "error", "拉取失败", String(cause)); }
  finally { loading.value = false; }
}
async function resolvePullConflict(strategy: "local" | "remote" | "ai") {
  if (!selected.value || !pullConflict.value) return;
  const conflict = pullConflict.value;
  resolvingConflict.value = true; error.value = "";
  try {
    const result = await resolveGitPullConflicts(selected.value.path, strategy, conflict.localHead, conflict.remoteHead);
    pullConflict.value = null;
    message.value = result.message;
    recordGitOperation("解决拉取冲突", "success", result.message, result.output);
    await refreshSelected();
  } catch (cause) { error.value = String(cause); recordGitOperation("解决拉取冲突", "error", "冲突处理失败", String(cause)); }
  finally { resolvingConflict.value = false; }
}
function closePullConflict() { if (!resolvingConflict.value) pullConflict.value = null; }
async function commitGroup(groupId: string, files: string[]) {
  if (!selected.value) return; const commitMessage = commitMessages[groupId]?.trim() ?? "";
  if (!files.length) return;
  const result = await runGitAction("提交修改", () => commitGitPlanGroup(selected.value!.path, groupId, commitMessage));
  if (result?.commitHash) postCommitPrompt.value = { commitHash: result.commitHash, message: commitMessage.split("\n")[0] || "提交已完成" };
}
async function saveCredential() {
  loading.value = true; error.value = "";
  try { await saveGitDefaultCredential(credentialUsername.value, credentialSecret.value); credentialSecret.value = ""; message.value = "默认 Git 凭据已保存到 Windows 凭据库。"; if (selected.value) await loadGitStatus(selected.value.path); }
  catch (cause) { error.value = String(cause); } finally { loading.value = false; }
}
async function clearCredential() {
  await clearGitDefaultCredential(); credentialSecret.value = ""; credentialUsername.value = "lzsk"; message.value = "工作台默认 Git 凭据已删除。"; if (selected.value) await loadGitStatus(selected.value.path);
}
async function stageSelectedFiles() {
  if (!selected.value || !selectedStageFiles.value.length) return;
  await runGitAction("添加到暂存区", () => stageGitRepositoryChanges(selected.value!.path, [...selectedStageFiles.value]));
}
async function previewFileDiff(file: GitChangedFile) {
  if (!selected.value || diffLoadingPath.value) return;
  diffLoadingPath.value = file.path; error.value = "";
  try { fileDiff.value = await getGitRepositoryFileDiff(selected.value.path, file.path); }
  catch (cause) { error.value = String(cause); }
  finally { diffLoadingPath.value = ""; }
}
async function unstageFile(file: GitChangedFile) {
  if (!selected.value) return;
  await runGitAction("移出暂存区", () => unstageGitRepositoryChanges(selected.value!.path, [file.path]));
}
async function pushAfterCommit() {
  if (!selected.value) return;
  const result = await runGitAction("推送提交", () => pushGitRepository(selected.value!.path));
  if (result) postCommitPrompt.value = null;
}
async function saveChangesTemporarily() {
  if (!selected.value) return;
  await runGitAction("临时保存修改", () => stashGitRepositoryChanges(selected.value!.path));
}
async function restoreTemporaryChanges() {
  if (!selected.value) return;
  await runGitAction("恢复临时修改", () => restoreGitRepositoryStash(selected.value!.path));
}
async function abortCurrentMerge() {
  if (!selected.value) return;
  await runGitAction("取消合并", () => abortGitRepositoryMerge(selected.value!.path));
}
async function save() {
  loading.value = true; error.value = "";
  try { await saveRepositoryAsset({ ...form }); message.value = "人工修正已保存，后续扫描不会覆盖这些说明。"; editing.value = false; await refreshSelected(); }
  catch (cause) { error.value = String(cause); } finally { loading.value = false; }
}
function exportText(format: "markdown" | "csv") {
  if (format === "csv") {
    const quote = (value: string | number | boolean) => `"${String(value).replaceAll('"', '""')}"`;
    return [["置顶", "项目", "路径", "分类", "用途", "当前分支", "运行状态", "健康状态", "修改文件", "待处理", "Codex任务", "提交记录"].map(quote).join(","), ...filtered.value.map((item) => [item.isPinned ? "是" : "否", item.name, item.path, item.category, item.purpose, item.defaultBranch, runtimeLabel(item.runtimeStatus), item.healthLevel, item.changedFileCount, item.pendingSummary, item.conversationCount, item.commitCount].map(quote).join(","))].join("\n");
  }
  return ["# 项目资产清单", "", `共 ${filtered.value.length} 个项目`, "", ...filtered.value.flatMap((item) => [`## ${item.isPinned ? "📌 " : ""}${item.name}`, `- 路径：${item.path}`, `- 分类：${item.category}`, `- 用途：${item.purpose || "待确认"}`, `- 当前分支：${item.defaultBranch || "无分支"}`, `- 运行：${runtimeLabel(item.runtimeStatus)}`, `- 健康：${item.healthLevel}`, `- 待处理：${item.pendingSummary}`, ""])].join("\n");
}
async function copyExport(format: "markdown" | "csv") { await navigator.clipboard.writeText(exportText(format)); message.value = `${format === "csv" ? "CSV" : "Markdown"} 清单已复制，可直接保存为文件。`; }
onMounted(async () => {
  await load();
  await openProjectFromRoute();
  runtimeTimer = window.setInterval(() => {
    nowTick.value = Date.now();
    void refreshRuntime();
  }, 2000);
});
onBeforeUnmount(() => { if (runtimeTimer) window.clearInterval(runtimeTimer); });
</script>

<template>
  <div class="view projects-view">
    <header class="page-header"><div><h1>项目资产</h1><p>从这里继续工作、运行项目、处理风险，并查看项目关联记录</p></div><div><RouterLink class="button secondary link-button" to="/project-mapping">项目映射</RouterLink><button class="button secondary" @click="showHiddenList = true">隐藏列表 {{ hiddenAssets.length }}</button><button class="button secondary" @click="copyExport('csv')">导出 CSV</button><button class="button secondary" @click="openScanConfiguration">扫描设置</button><button class="button primary" :disabled="loading" @click="scan">{{ loading ? "处理中…" : "↻ 重新扫描" }}</button></div></header>
    <div v-if="error || message" class="scan-message" :class="{ error: Boolean(error) }">{{ error || message }}</div>
    <section class="asset-metrics"><article class="panel running"><small>工作台运行中</small><b>{{ runningProcesses.length }}</b><span>启动、停止和日志都留在工作台</span></article><article class="panel warning"><small>需要处理</small><b>{{ attentionCount }}</b><span>修改、落后、启动失败或长期未维护</span></article><article class="panel"><small>有未提交修改</small><b>{{ dirtyCount }}</b><span>工作状态，不再算健康故障</span></article><article class="panel failed"><small>健康检查失败</small><b>{{ failedCount }}</b><span>只统计目录或 Git 无法读取</span></article></section>
    <section class="panel asset-workspace">
      <nav class="project-view-tabs"><button :class="{ active: viewMode === 'recent' }" @click="viewMode = 'recent'">最近使用</button><button :class="{ active: viewMode === 'active' }" @click="viewMode = 'active'">正在开发</button><button :class="{ active: viewMode === 'attention' }" @click="viewMode = 'attention'">需要处理</button><button :class="{ active: viewMode === 'stale' }" @click="viewMode = 'stale'">长期未维护</button></nav>
      <div class="asset-toolbar"><label>⌕<input v-model="query" placeholder="搜索项目、路径、用途或当前分支"></label><select v-model="categoryFilter"><option>全部分类</option><option v-for="item in categories" :key="item">{{ item }}</option></select><select v-model="health"><option>全部状态</option><option>健康</option><option>失败</option><option>未验证</option></select><select v-model="changes"><option>全部工作区</option><option>有未提交修改</option><option>工作区干净</option></select><span>智能排序 · {{ filtered.length }} / {{ visibleAssets.length }}</span></div>
      <div class="asset-table-wrap">
        <table class="asset-table">
          <thead><tr><th class="pin-column">置顶</th><th>项目</th><th>用途与分类</th><th>当前分支</th><th>工作状态</th><th>运行状态与地址</th><th>操作</th></tr></thead>
          <tbody>
            <tr v-for="item in filtered" :key="item.path" :class="{ pinned: item.isPinned }" @click="openAsset(item)">
              <td><button class="pin-button" :class="{ active: item.isPinned }" :title="item.isPinned ? '取消置顶' : '置顶项目'" @click.stop="togglePin(item)">{{ item.isPinned ? "★" : "☆" }}</button></td>
              <td><b>{{ item.name }}</b><small>{{ item.path }}</small></td>
              <td><button class="category-button" title="点击修改分类" @click.stop="openCategoryEditor(item)">{{ item.category }}</button><small>{{ projectDescription(item) }}</small></td>
              <td><b class="branch-name">{{ item.defaultBranch || "无分支" }}</b><small v-if="branchSyncSummary(item)" class="branch-sync" :class="{ behind: item.behindCount > 0 }">{{ branchSyncSummary(item) }}</small></td>
              <td><div class="work-state"><i v-if="item.healthLevel !== '健康'" class="health-pill" :class="healthClass(item.healthLevel)">健康：{{ item.healthLevel }}</i><i v-if="item.changedFileCount" class="dirty-pill">{{ item.changedFileCount }} 个文件修改</i><small v-if="item.healthLevel === '健康' && !item.changedFileCount" class="clean-state">工作区干净</small></div></td>
              <td><div class="runtime-cell"><i class="runtime-pill" :class="runtimeClass(runtimeMap.get(item.path)?.status || item.runtimeStatus)">{{ runtimeLabel(runtimeMap.get(item.path)?.status || item.runtimeStatus) }}</i><button v-if="projectRuntimeUrl(item)" class="runtime-address-button" :title="`使用默认浏览器打开 ${projectRuntimeUrl(item)}`" @click.stop="openRuntimeUrl(projectRuntimeUrl(item))">{{ projectRuntimeUrl(item) }}</button><small v-else class="runtime-address-empty">{{ isProjectRunning(item.path) ? '正在获取运行地址…' : '—' }}</small></div></td>
              <td><div class="asset-row-actions"><button class="asset-row-button launch" :class="{ stop: isProjectRunning(item.path) }" :disabled="Boolean(startingPath)" @click.stop="toggleProjectProcess(item)">{{ startingPath === item.path ? (isProjectRunning(item.path) ? '停止中…' : '启动中…') : (isProjectRunning(item.path) ? '■ 停止' : '▶ 启动') }}</button><button class="asset-row-button git" @click.stop="openAsset(item, 'git')">Git</button></div></td>
            </tr>
          </tbody>
        </table>
        <div v-if="!filtered.length" class="empty-state"><b>当前视图没有项目</b><p>可以切换快捷视图或调整筛选条件。</p></div>
      </div>
    </section>
    <div v-if="showHiddenList" class="activity-backdrop hidden-project-backdrop" @click.self="showHiddenList = false"><section class="hidden-project-dialog panel"><header><div><h2>隐藏项目</h2><p>隐藏只影响项目资产列表，不会删除仓库、报告或历史数据。</p></div><button class="icon-button" @click="showHiddenList = false">×</button></header><div class="hidden-project-list"><article v-for="item in hiddenAssets" :key="item.path"><div><b>{{ item.name }}</b><small>{{ item.path }}</small></div><button class="button secondary small" @click="toggleHidden(item, false)">恢复显示</button></article><div v-if="!hiddenAssets.length" class="empty-state"><b>没有隐藏项目</b><p>在项目详情右上角点击“隐藏”后，会显示在这里。</p></div></div></section></div>
    <div v-if="categoryEditor" class="activity-backdrop category-editor-backdrop" @click.self="categoryEditor = null"><form class="panel category-editor-dialog" @submit.prevent="saveQuickCategory"><header><div><h2>修改项目分类</h2><p>{{ categoryEditor.name }}</p></div><button type="button" class="icon-button" @click="categoryEditor = null">×</button></header><label>分类名称<input v-model="categoryDraft" maxlength="40" list="project-category-options" autofocus placeholder="例如：业务系统 / 个人工具 / 视频项目"><datalist id="project-category-options"><option v-for="item in categories" :key="item" :value="item"></option></datalist></label><footer><button type="button" class="button secondary" @click="categoryEditor = null">取消</button><button class="button primary" :disabled="categorySaving || !categoryDraft.trim()">{{ categorySaving ? '保存中…' : '保存分类' }}</button></footer></form></div>
    <div v-if="showScanSettings" class="activity-backdrop scan-settings-backdrop" @click.self="showScanSettings = false"><section class="panel scan-settings-dialog"><header><div><h2>Git 扫描设置</h2><p>控制项目资产从哪些目录发现仓库；保存设置不会修改项目文件。</p></div><button class="icon-button" @click="showScanSettings = false">×</button></header><div class="scan-settings-body"><label>扫描深度 <input v-model.number="scanConfiguration.maxDepth" type="number" min="1" max="6"><small>从每个根目录向下扫描 1–6 层。</small></label><section><b>扫描根目录</b><div class="scan-chip-list"><span v-for="(root, index) in scanConfiguration.roots" :key="root"><code>{{ root }}</code><button title="移除" @click="scanConfiguration.roots.splice(index, 1)">×</button></span></div><div class="scan-add-row"><input v-model="newScanRoot" placeholder="例如 F:\TB-project" @keyup.enter="addScanRoot"><button class="button secondary" @click="addScanRoot">添加目录</button></div></section><section><b>额外排除的目录名</b><div class="scan-chip-list"><span v-for="(name, index) in scanConfiguration.excludedNames" :key="name"><code>{{ name }}</code><button title="移除" @click="scanConfiguration.excludedNames.splice(index, 1)">×</button></span><small v-if="!scanConfiguration.excludedNames.length">仍会默认跳过 node_modules、target、dist、build 等依赖或构建目录。</small></div><div class="scan-add-row"><input v-model="newExcludedName" placeholder="例如 archive" @keyup.enter="addExcludedName"><button class="button secondary" @click="addExcludedName">添加排除项</button></div></section><section class="scan-history"><b>最近扫描</b><p>{{ formatTime(latestScanAt) }}</p><ul v-if="lastScanErrors.length"><li v-for="item in lastScanErrors" :key="item">{{ item }}</li></ul><small v-else>本次工作台会在扫描结束后列出无法读取的具体目录。</small></section></div><footer><button class="button secondary" :disabled="scanSaving" @click="persistScanConfiguration(false)">仅保存</button><button class="button primary" :disabled="scanSaving || !scanConfiguration.roots.length" @click="persistScanConfiguration(true)">{{ scanSaving ? "保存中…" : "保存并重新扫描" }}</button></footer></section></div>
    <div v-if="pullConflict" class="activity-backdrop pull-conflict-backdrop" @click.self="closePullConflict"><section class="panel pull-conflict-dialog">
      <header><div><h2>拉取代码发现冲突</h2><p>仓库已恢复到拉取前状态，请选择冲突部分的处理方式。</p></div><button class="icon-button" :disabled="resolvingConflict" @click="closePullConflict">×</button></header>
      <div class="pull-conflict-body">
        <section><b>{{ pullConflict.files.length }} 个冲突文件</b><ul><li v-for="file in pullConflict.files" :key="file"><code>{{ file }}</code></li></ul></section>
        <div class="pull-conflict-options">
          <button :disabled="resolvingConflict" @click="resolvePullConflict('local')"><b>保留我的</b><span>冲突位置采用本地内容，线上不冲突的修改仍会保留。</span></button>
          <button :disabled="resolvingConflict" @click="resolvePullConflict('remote')"><b>保留线上</b><span>冲突位置采用线上内容，本地不冲突的修改仍会保留。</span></button>
          <button class="ai" :disabled="resolvingConflict || pullConflict.aiBlockedFiles.length > 0" @click="resolvePullConflict('ai')"><b>AI 智能合并 · 保留双方</b><span>AI 按语义合并双方文本修改，完成后创建合并提交但不会推送。</span></button>
        </div>
        <p v-if="pullConflict.aiBlockedFiles.length" class="pull-conflict-warning">以下文件包含敏感信息、二进制/生成内容、内容过长或冲突文件数量过多，不能使用 AI 自动合并：{{ pullConflict.aiBlockedFiles.join("、") }}</p>
        <p v-if="resolvingConflict" class="pull-conflict-progress">正在处理冲突，请不要关闭工作台…</p>
      </div>
      <footer><button class="button secondary" :disabled="resolvingConflict" @click="closePullConflict">暂不处理</button></footer>
    </section></div>
    <div v-if="selected" class="activity-backdrop" @click.self="selected = null"><aside class="asset-drawer panel">
      <header><div class="drawer-project-title"><small>PROJECT ASSET</small><h2>{{ selected.name }}</h2><p>{{ selected.path }}</p></div><div class="drawer-header-actions"><button class="drawer-action-button continue" @click="continueWork">继续工作</button><button class="drawer-action-button" @click="router.push({path:'/api-docs',query:{projectPath:selected!.path}})">接口文档</button><button class="drawer-action-button primary" :class="{ stop: isProjectRunning(selected.path) }" :disabled="Boolean(startingPath)" @click="toggleProjectProcess(selected)">{{ startingPath === selected.path ? (isProjectRunning(selected.path) ? "停止中…" : "启动中…") : (isProjectRunning(selected.path) ? "■ 停止" : "▶ 运行") }}</button><button class="drawer-action-button" @click="activeTab = 'overview'; editing = true">编辑</button><button class="drawer-action-button" @click="toggleHidden(selected, true)">隐藏</button><button class="drawer-action-button" :class="{ active: selected.isPinned }" @click="togglePin(selected)">{{ selected.isPinned ? "★ 已置顶" : "☆ 置顶" }}</button><button class="drawer-action-button close" title="关闭" @click="selected = null">×</button></div></header>
      <div class="asset-status-strip"><span class="runtime-pill" :class="runtimeClass(selectedRuntime?.status)">{{ runtimeLabel(selectedRuntime?.status) }}</span><span class="health-pill" :class="healthClass(selected.healthLevel)">健康：{{ selected.healthLevel }}</span><span>分支 {{ gitStatus?.currentBranch || selected.defaultBranch || "无分支" }}</span><span>{{ gitStatus?.changedFiles.length || selected.changedFileCount }} 个文件有修改</span><span v-if="selected.behindCount">落后 {{ selected.behindCount }}</span></div>
      <nav class="asset-tabs"><button :class="{ active: activeTab === 'overview' }" @click="activeTab = 'overview'">项目说明</button><button :class="{ active: activeTab === 'relations' }" @click="activeTab = 'relations'">关联中心 {{ details.associations.length }}</button><button :class="{ active: activeTab === 'git' }" :disabled="autoFetchingRemote" @click="openGitOperations">{{ autoFetchingRemote ? "更新远程中…" : "Git 操作" }}</button></nav>
      <template v-if="activeTab === 'overview'">
        <template v-if="!editing"><section class="runtime-console"><header><div><h3>运行状态</h3><p>项目在工作台后台运行，不自动打开 HBuilderX、浏览器或命令行窗口。</p></div><i class="runtime-pill large" :class="runtimeClass(selectedRuntime?.status)">{{ runtimeLabel(selectedRuntime?.status) }}</i></header><div class="runtime-metrics"><article><small>启动时间</small><b>{{ formatTime(selectedRuntime?.startedAt) }}</b></article><article><small>已运行</small><b>{{ isProjectRunning(selected.path) ? runtimeDuration(selectedRuntime?.startedAt) : "—" }}</b></article><article><small>进程号</small><b>{{ selectedRuntime?.processId || "—" }}</b></article></div><div v-if="selectedRuntime?.localUrl" class="runtime-url"><code>{{ selectedRuntime.localUrl }}</code><button class="button primary small" @click="openRuntimeUrl(selectedRuntime.localUrl)">打开本地页面</button></div><p v-if="selectedRuntime?.errorMessage" class="runtime-error">{{ selectedRuntime.errorMessage }}</p><div class="runtime-log"><header><b>最近日志</b><div><button class="text-button" :disabled="!selectedRuntime?.logExcerpt" @click="copyRuntimeLog">复制日志</button><small v-if="selectedRuntime?.logPath">{{ selectedRuntime.logPath }}</small></div></header><pre>{{ selectedRuntime?.logExcerpt || "启动项目后，编译输出会显示在这里。" }}</pre></div></section><section class="continue-card"><div><small>建议下一步</small><b>{{ selected.nextAction }}</b><p>{{ details.nextAction }}</p></div><button class="button primary" @click="continueWork">继续工作</button></section><section class="asset-overview"><header><h3>项目说明</h3></header><dl><div><dt>分类</dt><dd>{{ selected.category }}</dd></div><div><dt>用途</dt><dd>{{ selected.purpose || "尚未填写项目用途" }}</dd></div><div><dt>技术栈</dt><dd>{{ selected.technologyStack || "首次启动时自动识别" }}</dd></div><div><dt>主要模块</dt><dd>{{ selected.mainModules || "尚未整理主要模块" }}</dd></div></dl></section><details class="asset-commands"><summary><span>运行配置</span><small>{{ selected.startCommand || "首次启动时自动识别" }}</small></summary><div v-for="[label, value] in [['安装', selected.installCommand], ['启动', selected.startCommand], ['测试', selected.testCommand], ['构建', selected.buildCommand]]" :key="label"><span>{{ label }}</span><code>{{ value || "未配置" }}</code></div><small>来源：{{ selected.commandSource || "尚未识别" }}。工作台不会从 README 任意执行命令。</small></details></template>
        <form v-else class="asset-form" @submit.prevent="save"><h3>编辑项目资料</h3><label>项目分类<input v-model="form.category" placeholder="例如：业务系统 / 工具 / 视频工程"></label><label>项目用途<textarea v-model="form.purpose" rows="3" placeholder="一句话说明这个项目解决什么问题"></textarea></label><details class="asset-form-advanced"><summary>高级资料与运行命令</summary><label>技术栈<input v-model="form.technologyStack" placeholder="例如：Vue 3 / Vite / Element Plus"></label><label>主要模块<textarea v-model="form.mainModules" rows="2" placeholder="用逗号分隔主要功能模块"></textarea></label><div class="form-grid"><label>安装命令<input v-model="form.installCommand"></label><label>启动命令<input v-model="form.startCommand" placeholder="留空时启动会自动识别"></label><label>测试命令<input v-model="form.testCommand"></label><label>构建命令<input v-model="form.buildCommand"></label></div><label>命令来源<input v-model="form.commandSource" placeholder="例如：package.json scripts（已核对）"></label></details><footer><button type="button" class="button secondary" @click="editing = false">取消</button><button class="button primary" :disabled="loading">保存资料</button></footer></form>
        <section class="asset-related"><div><h3>最近 Codex 任务</h3><button v-for="item in details.conversations.slice(0, 5)" :key="item.id" @click="router.push({ path: '/tokens', query: { conversation: item.id } })"><span><b>{{ item.title }}</b><small>{{ item.updatedAt.slice(0, 10) }}</small></span><em>继续 →</em></button><p v-if="!details.conversations.length">暂无直接关联任务。</p></div><div><h3>最近 Git 提交</h3><article v-for="item in details.commits.slice(0, 5)" :key="item.hash"><code>{{ item.hash.slice(0, 7) }}</code><span><b>{{ item.subject }}</b><small>{{ item.committedAt.slice(0, 10) }}</small></span></article><p v-if="!details.commits.length">暂无可读取提交。</p></div></section>
      </template>
      <template v-else-if="activeTab === 'git'">
        <section v-if="gitStatus" class="git-dashboard">
          <header><div><h3>仓库状态</h3><p>{{ gitStatus.remoteUrl || "未配置 origin 远程仓库" }}</p></div><button class="button secondary small" :disabled="loading" @click="loadGitStatus(selected.path)">刷新状态</button></header>
          <div class="git-metrics"><article><small>当前分支</small><b>{{ gitStatus.currentBranch }}</b></article><article><small>上游分支</small><b>{{ gitStatus.upstream || "未关联" }}</b></article><article><small>领先 / 落后</small><b>{{ gitStatus.ahead }} / {{ gitStatus.behind }}</b></article><article><small>工作区</small><b>{{ gitStatus.changedFiles.length }} 个文件</b></article></div>
          <div class="git-remote-actions">
            <button class="button secondary" :disabled="loading || !selectedStageFiles.length" @click="stageSelectedFiles">添加选中文件<span v-if="selectedStageFiles.length">（{{ selectedStageFiles.length }}）</span></button>
            <button class="button secondary" :disabled="loading || autoFetchingRemote || !gitStatus.remoteUrl" @click="runGitAction('更新远程', () => fetchGitRepository(selected!.path))">{{ autoFetchingRemote ? "更新中…" : "更新远程" }}</button>
            <button class="button secondary" :class="{ 'remote-update-available': gitStatus.behind > 0 }" :disabled="loading || !gitStatus.remoteUrl" :title="gitStatus.behind > 0 ? `远程有 ${gitStatus.behind} 个新提交` : '当前没有可拉取的新代码'" @click="pullRepository">{{ gitStatus.behind > 0 ? `拉取代码（${gitStatus.behind}）` : "拉取代码" }}</button>
            <button class="button primary" :disabled="loading || !canPush" :title="canPush ? (gitStatus.upstream ? `有 ${gitStatus.ahead} 个提交待推送` : '首次推送并建立上游分支') : '当前没有待推送提交'" @click="pushAfterCommit">推送</button>
          </div>
          <small class="git-action-help">打开 Git 操作时会自动更新一次远程状态；先从下方勾选文件再添加到 Git 暂存区；拉取会快进或创建合并提交，冲突时自动中止并恢复；只有存在尚未推送的本地提交时才能推送。</small>
          <small v-if="remoteRefreshNote" class="remote-refresh-note" :class="{ updates: gitStatus.behind > 0 }">{{ remoteRefreshNote }}</small>
          <details v-if="gitStatus.changedFiles.length" class="git-changed-files" open>
            <summary>查看 {{ gitStatus.changedFiles.length }} 个变更文件</summary>
            <label v-if="stageableFiles.length" class="git-stage-select-all"><input type="checkbox" :checked="allStageableFilesSelected" @change="toggleAllStageableFiles">全选可添加文件<span>已选择 {{ selectedStageFiles.length }} 个</span></label>
            <article v-for="file in gitStatus.changedFiles" :key="`${file.indexStatus}${file.worktreeStatus}:${file.path}`">
              <input v-if="hasUnstagedFile(file)" v-model="selectedStageFiles" type="checkbox" :value="file.path" :aria-label="`选择 ${file.path}`">
              <span v-else class="git-stage-check">✓</span>
              <i>{{ file.label }}</i>
              <code>{{ file.path }}</code>
              <em :class="{ staged: hasStagedFile(file) }">{{ stageFileLabel(file) }}</em>
              <span class="git-file-actions"><button class="text-button" :disabled="Boolean(diffLoadingPath)" @click="previewFileDiff(file)">{{ diffLoadingPath === file.path ? "读取中…" : "查看差异" }}</button><button v-if="hasStagedFile(file)" class="text-button" :disabled="loading" @click="unstageFile(file)">移出暂存区</button></span>
            </article>
          </details>
          <section v-if="fileDiff" class="git-diff-preview">
            <header><div><h4>文件差异</h4><code>{{ fileDiff.path }}</code></div><button class="text-button" @click="fileDiff = null">关闭</button></header>
            <p v-if="fileDiff.truncated" class="diff-warning">文件差异过长，当前只显示前面部分。</p>
            <div v-if="fileDiff.stagedDiff" class="diff-section"><b>已暂存修改</b><div class="diff-code"><span v-for="(line, index) in fileDiff.stagedDiff.split('\n')" :key="`staged-${index}`" :class="diffLineClass(line)">{{ line || " " }}</span></div></div>
            <div v-if="fileDiff.unstagedDiff" class="diff-section"><b>未暂存修改</b><div class="diff-code"><span v-for="(line, index) in fileDiff.unstagedDiff.split('\n')" :key="`unstaged-${index}`" :class="diffLineClass(line)">{{ line || " " }}</span></div></div>
            <p v-if="!fileDiff.stagedDiff && !fileDiff.unstagedDiff" class="git-empty">当前文件没有可显示的文本差异。</p>
          </section>
        </section>
        <section class="commit-plan">
          <header><div><h3>提交修改</h3><p>AI 根据实际代码差异生成中文提交信息；提交前可以修改。</p></div><div class="commit-plan-actions"><div class="commit-grouping-switch" role="group" aria-label="提交分组方式"><button :class="{ active: commitGroupingMode === 'single' }" @click="commitGroupingMode = 'single'">全部合成一次</button><button :class="{ active: commitGroupingMode === 'feature' }" @click="commitGroupingMode = 'feature'">按功能关联分组</button></div><button class="button secondary" :disabled="loading || !gitStatus?.hasUncommittedChanges" @click="planCommit">{{ loading ? "AI 分析中…" : details.commitPlan ? "重新生成" : "AI 生成提交建议" }}</button></div></header>
          <p v-if="!gitStatus?.hasUncommittedChanges" class="git-empty">当前工作区干净，没有可提交修改。</p>
          <div v-if="details.commitPlan && gitStatus?.hasUncommittedChanges" class="commit-plan-summary"><div class="commit-plan-meta"><span>风险 {{ details.commitPlan.riskLevel }}</span><span :class="{ fallback: details.commitPlan.generator === 'rules' }">{{ details.commitPlan.generator === "deepseek" ? `AI 生成 · ${details.commitPlan.model}` : "本地规则降级" }}</span><span>{{ details.commitPlan.groupingMode === "single" ? "全部合成一次" : "按功能关联分组" }}</span></div><p>{{ details.commitPlan.summary }}</p><p v-if="details.commitPlan.generationWarning" class="commit-generation-warning">{{ details.commitPlan.generationWarning }}</p><details v-if="details.commitPlan.excludedFiles.length" class="commit-excluded-files"><summary>{{ details.commitPlan.excludedFiles.length }} 个敏感文件、二进制或生成物未纳入提交</summary><p v-for="file in details.commitPlan.excludedFiles" :key="file">{{ file }}</p></details><article v-for="group in details.commitPlan.groups" :key="group.id"><div class="commit-group-title"><b>{{ group.title }}</b><i :class="{ committed: group.status === 'committed' }">{{ group.status === "committed" ? "已提交" : `${group.files.length} 个文件` }}</i></div><textarea v-model="commitMessages[group.id]" rows="4" :disabled="group.status === 'committed'" aria-label="提交信息和修改明细"></textarea><small class="commit-message-help">第一行为提交标题，空一行后每项修改使用“- ”开头；这些内容仍会合成一次提交。</small><small>{{ group.riskNotes }}</small><details><summary>查看文件</summary><p v-for="file in group.files" :key="file">{{ file }}</p></details><button class="button primary small" :disabled="loading || group.status === 'committed'" @click="commitGroup(group.id, group.files)">{{ details.commitPlan.groupingMode === "single" ? "提交全部修改" : "提交本组" }}</button></article></div>
        </section>
        <section v-if="postCommitPrompt" class="post-commit-guide"><div><small>提交已完成</small><b>{{ postCommitPrompt.message }}</b><code>{{ postCommitPrompt.commitHash.slice(0, 7) }}</code><p>提交只保存在本地，确认后可以推送到远程。</p></div><div><button class="button secondary" @click="postCommitPrompt = null">继续修改</button><button class="button primary" :disabled="loading || !canPush" @click="pushAfterCommit">推送到远程</button></div></section>
        <section v-if="gitStatus" class="git-safety-panel"><header><div><h3>安全撤销</h3><p>这些操作不会删除提交历史；临时保存会包含未跟踪文件。</p></div></header><div><button class="button secondary" :disabled="loading || !gitStatus.hasUncommittedChanges || gitStatus.mergeInProgress" @click="saveChangesTemporarily">临时保存修改</button><button class="button secondary" :disabled="loading || gitStatus.hasUncommittedChanges || !gitStatus.hasWorkbenchStash" @click="restoreTemporaryChanges">恢复临时修改</button><button class="button secondary" :disabled="loading || !gitStatus.mergeInProgress" @click="abortCurrentMerge">取消当前合并</button></div><small>已暂存文件可在上方文件列表中逐个“移出暂存区”，本地修改仍会保留。</small></section>
        <section v-if="gitStatus" class="git-branch-panel"><h3>分支操作</h3><div><label>切换本地分支<select v-model="switchBranch"><option v-for="branch in gitStatus.branches" :key="branch">{{ branch }}</option></select></label><button class="button secondary" :disabled="loading || switchBranch === gitStatus.currentBranch" @click="runGitAction('切换分支', () => switchGitRepositoryBranch(selected!.path, switchBranch))">切换</button></div><div><label>合并来源分支<select v-model="mergeBranch"><option value="" disabled>请选择分支</option><option v-for="branch in mergeOptions" :key="branch">{{ branch }}</option></select></label><button class="button secondary" :disabled="loading || !mergeBranch" @click="runGitAction('合并分支', () => mergeGitRepositoryBranch(selected!.path, mergeBranch))">合并</button></div></section>
        <section class="git-credential-panel"><header><div><h3>默认 Git 凭据</h3><p>项目自身没有可用登录凭据时使用；只保存在 Windows 凭据库。</p></div><span :class="{ configured: gitStatus?.credential.configured }">{{ gitStatus?.credential.configured ? `已配置 · ${gitStatus.credential.username}` : "未配置" }}</span></header><div><input v-model="credentialUsername" placeholder="用户名（默认 lzsk）"><input v-model="credentialSecret" type="password" autocomplete="new-password" placeholder="密码或访问令牌"><button class="button primary" :disabled="loading || !credentialSecret" @click="saveCredential">保存凭据</button><button v-if="gitStatus?.credential.configured" class="button danger-button" @click="clearCredential">删除</button></div><small>GitHub 已不支持账号密码拉取，请在这里填写个人访问令牌；密码或令牌不会写入项目配置、数据库或日志。</small></section>
        <section class="git-history"><header><div><h3>提交历史与回退</h3><p>回退会创建一条新的 revert 提交，不改写已有历史。</p></div></header><article v-for="item in details.commits" :key="item.hash"><code>{{ item.hash.slice(0, 7) }}</code><span><b>{{ item.subject }}</b><small>{{ item.committedAt.slice(0, 10) }}</small></span><button class="text-button danger-text" :disabled="loading" @click="runGitAction('回退提交', () => revertGitRepositoryCommit(selected!.path, item.hash))">回退</button></article><p v-if="!details.commits.length" class="git-empty">暂无可读取提交历史。</p></section>
        <section class="git-operation-log"><header><div><h3>操作结果与日志</h3><p>保留当前项目最近 20 次 Git 操作，切换项目后自动清空。</p></div><button v-if="gitOperationLogs.length" class="text-button" @click="gitOperationLogs = []">清空显示</button></header><article v-for="entry in gitOperationLogs" :key="entry.id" :class="entry.status"><div><i>{{ entry.status === "success" ? "成功" : "失败" }}</i><b>{{ entry.action }}</b><time>{{ formatTime(entry.occurredAt) }}</time></div><p>{{ entry.message }}</p><details v-if="entry.output"><summary>查看详细日志</summary><pre>{{ entry.output }}</pre></details></article><p v-if="!gitOperationLogs.length" class="git-empty">执行暂存、拉取、提交、推送或撤销操作后，结果会显示在这里。</p></section>
      </template>
      <template v-else>
        <section class="association-center"><header><div><h3>项目关联中心</h3><p>把任务、缺陷、测试、工作记录、报告、文档和构建入口集中在一个地方。</p></div><span>{{ details.associations.length }} 条关联</span></header><div class="association-grid"><section v-for="group in associationGroups" :key="group.kind"><header><b>{{ group.label }}</b><span>{{ group.items.length }}</span></header><button v-for="item in group.items" :key="`${item.kind}:${item.id}`" :disabled="!item.route" @click="openAssociation(item.route)"><i>{{ item.kind.toUpperCase() }}</i><span><b>{{ item.title }}</b><small>{{ item.subtitle }}<template v-if="item.updatedAt"> · {{ formatTime(item.updatedAt) }}</template></small></span><em>{{ item.route ? "查看 →" : item.status }}</em></button></section><div v-if="!details.associations.length" class="empty-state"><b>尚未发现关联记录</b><p>执行测试、记录工时、关联 TAPD 或继续 Codex 任务后会自动出现在这里。</p></div></div></section>
      </template>
    </aside></div>
  </div>
</template>

<style scoped>
.asset-metrics{display:grid;grid-template-columns:repeat(4,1fr);gap:12px;margin-bottom:12px}.asset-metrics article{padding:15px 17px;min-height:92px}.asset-metrics small,.asset-metrics span{color:var(--muted)}.asset-metrics b{display:block;font-size:27px;margin:7px 0}.asset-metrics span{font-size:9px}.asset-metrics .warning b{color:var(--warning)}.asset-metrics .failed b{color:var(--danger)}.asset-workspace{overflow:hidden}.asset-toolbar{height:58px;border-bottom:1px solid var(--line);display:flex;align-items:center;gap:9px;padding:0 14px}.asset-toolbar label{flex:1;max-width:410px;height:35px;border:1px solid var(--line);border-radius:7px;display:flex;align-items:center;gap:8px;padding:0 10px;color:var(--muted)}.asset-toolbar input{flex:1;border:0;outline:0;background:transparent}.asset-toolbar select{height:35px;border:1px solid var(--line);border-radius:7px;background:var(--surface-2);padding:0 10px}.asset-toolbar>span{margin-left:auto;color:var(--muted)}.asset-table-wrap{max-height:590px;overflow:auto}.asset-table{width:100%;border-collapse:collapse}.asset-table th{text-align:left;color:var(--muted);font-size:9px;background:var(--surface-2);position:sticky;top:0;z-index:2}.asset-table th,.asset-table td{padding:11px 12px;border-bottom:1px solid var(--line)}.asset-table th.pin-column{width:54px}.asset-table tbody tr{cursor:pointer}.asset-table tbody tr:hover,.asset-table tbody tr.pinned{background:var(--primary-soft)}.asset-table td b,.asset-table td small{display:block;max-width:240px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.asset-table td small{color:var(--muted);font-size:9px;margin-top:5px}.pin-button{min-width:34px;height:30px;border:1px solid var(--line);border-radius:7px;background:var(--surface-2);color:var(--muted);padding:0 9px}.pin-button.active{color:var(--warning);border-color:color-mix(in srgb,var(--warning) 45%,var(--line));background:color-mix(in srgb,var(--warning) 10%,var(--surface))}.health-pill,.dirty-pill{display:inline-flex;padding:5px 8px;border-radius:12px;font-style:normal;font-size:9px}.health-pill.healthy{color:var(--success);background:color-mix(in srgb,var(--success) 12%,transparent)}.health-pill.warning,.dirty-pill{color:var(--warning);background:color-mix(in srgb,var(--warning) 12%,transparent)}.health-pill.failed{color:var(--danger);background:color-mix(in srgb,var(--danger) 12%,transparent)}.health-pill.unknown{color:var(--muted);background:var(--surface-2)}.clean-text{color:var(--success);font-size:10px}.asset-drawer{width:780px;height:100%;margin-left:auto;border-radius:0;overflow:auto;padding-bottom:30px}.asset-drawer>header{padding:18px 20px;border-bottom:1px solid var(--line);display:flex;justify-content:space-between}.asset-drawer h2{margin:4px 0}.asset-drawer header p,.asset-drawer header small{margin:0;color:var(--muted)}.drawer-header-actions{display:flex;align-items:flex-start;gap:8px}.asset-status-strip{display:flex;gap:8px;align-items:center;padding:12px 20px;background:var(--surface-2);border-bottom:1px solid var(--line)}.asset-status-strip>span:not(.health-pill){padding:5px 8px;border:1px solid var(--line);border-radius:12px;font-size:9px;color:var(--muted)}.asset-tabs{height:48px;padding:6px 20px;border-bottom:1px solid var(--line);display:flex;gap:6px}.asset-tabs button{height:34px;border:0;border-radius:7px;background:transparent;color:var(--muted);padding:0 16px}.asset-tabs button.active{background:var(--primary-soft);color:var(--primary);font-weight:800}.asset-overview,.asset-commands,.asset-form{padding:18px 20px;border-bottom:1px solid var(--line)}.asset-drawer h3{margin:0 0 13px}.asset-overview dl{display:grid;grid-template-columns:1fr 1fr;gap:9px}.asset-overview dl div{padding:11px;background:var(--surface-2);border-radius:7px}.asset-overview dt{color:var(--muted);font-size:9px}.asset-overview dd{margin:6px 0 0;line-height:1.5}.asset-commands>div{display:grid;grid-template-columns:55px 1fr;align-items:center;margin-bottom:7px}.asset-commands code{padding:8px;background:var(--surface-2);border-radius:6px}.asset-commands small{color:var(--muted)}.asset-form label{display:flex;flex-direction:column;gap:6px;color:var(--muted);font-size:10px;margin-bottom:11px}.asset-form input,.asset-form textarea{border:1px solid var(--line);border-radius:7px;background:var(--surface-2);padding:9px;outline:0}.asset-form input:focus,.asset-form textarea:focus{border-color:var(--primary)}.asset-form footer{display:flex;justify-content:flex-end;gap:8px}.asset-related{display:grid;grid-template-columns:1fr 1fr;gap:12px;padding:18px 20px}.asset-related>div{min-width:0}.asset-related button,.asset-related article{width:100%;border:1px solid var(--line);border-radius:7px;background:var(--surface-2);color:inherit;padding:9px;margin-bottom:7px;display:flex;align-items:center;gap:9px;text-align:left}.asset-related button span,.asset-related article span{min-width:0;flex:1;display:flex;flex-direction:column;gap:4px}.asset-related b{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.asset-related small,.asset-related em,.asset-related p{color:var(--muted);font-size:9px}.asset-related em{font-style:normal}.asset-related article code{color:var(--primary)}
.asset-row-actions{display:flex;align-items:center;gap:8px;white-space:nowrap}.muted-action{color:var(--muted)}.row-launch-button{height:29px;border:1px solid color-mix(in srgb,var(--primary) 45%,var(--line));border-radius:7px;background:var(--primary-soft);color:var(--primary);padding:0 9px;font-weight:700;white-space:nowrap}.row-launch-button:disabled{opacity:.45;cursor:not-allowed}.category-button{max-width:150px;border:0;border-radius:99px;background:var(--primary-soft);color:var(--primary);padding:5px 9px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.hidden-project-backdrop,.category-editor-backdrop{z-index:230;align-items:center;justify-content:center;padding:30px}.hidden-project-dialog{width:min(720px,calc(100vw - 80px));max-height:min(720px,calc(100vh - 80px));overflow:hidden}.hidden-project-dialog>header{height:72px;padding:0 18px;border-bottom:1px solid var(--line);display:flex;align-items:center;justify-content:space-between}.hidden-project-dialog h2{margin:0 0 5px}.hidden-project-dialog header p{margin:0;color:var(--muted)}.hidden-project-list{max-height:620px;overflow:auto;padding:8px 18px 18px}.hidden-project-list article{min-height:68px;border-bottom:1px solid var(--line);display:flex;align-items:center;gap:12px}.hidden-project-list article>div{min-width:0;flex:1;display:flex;flex-direction:column;gap:6px}.hidden-project-list small{color:var(--muted);white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.category-editor-dialog{width:min(520px,calc(100vw - 80px));overflow:hidden}.category-editor-dialog>header{height:72px;padding:0 18px;border-bottom:1px solid var(--line);display:flex;align-items:center;justify-content:space-between}.category-editor-dialog h2{margin:0 0 5px}.category-editor-dialog header p{margin:0;color:var(--muted)}.category-editor-dialog>label{display:flex;flex-direction:column;gap:8px;padding:20px;color:var(--muted)}.category-editor-dialog input{height:40px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);padding:0 11px;outline:0}.category-editor-dialog input:focus{border-color:var(--primary)}.category-editor-dialog>footer{display:flex;justify-content:flex-end;gap:8px;padding:0 20px 20px}
.git-dashboard,.git-branch-panel,.git-credential-panel,.commit-plan,.git-history,.git-output{padding:18px 20px;border-bottom:1px solid var(--line)}.git-dashboard>header,.git-credential-panel>header,.commit-plan>header,.git-history>header{display:flex;align-items:flex-start;justify-content:space-between;gap:12px}.git-dashboard header p,.git-credential-panel header p,.commit-plan header p,.git-history header p{margin:4px 0 0;color:var(--muted);font-size:10px;overflow-wrap:anywhere}.git-metrics{display:grid;grid-template-columns:repeat(4,1fr);gap:8px;margin:13px 0}.git-metrics article{min-width:0;padding:11px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2)}.git-metrics small{display:block;color:var(--muted);font-size:9px}.git-metrics b{display:block;margin-top:7px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.git-remote-actions{display:flex;align-items:center;gap:8px}.git-remote-actions small{flex:1;color:var(--muted);line-height:1.5}.git-branch-panel>div{display:grid;grid-template-columns:1fr auto;gap:8px;margin-top:9px}.git-branch-panel label{display:grid;grid-template-columns:110px 1fr;align-items:center;gap:8px;color:var(--muted)}.git-branch-panel select,.git-credential-panel input{height:36px;border:1px solid var(--line);border-radius:7px;background:var(--surface-2);padding:0 10px;outline:0}.git-credential-panel>header>span{padding:5px 8px;border-radius:6px;background:var(--surface-2);color:var(--muted);font-size:9px}.git-credential-panel>header>span.configured{color:var(--success);background:color-mix(in srgb,var(--success) 12%,transparent)}.git-credential-panel>div{display:grid;grid-template-columns:140px minmax(180px,1fr) auto auto;gap:8px;margin:13px 0 8px}.git-credential-panel>small{color:var(--muted);line-height:1.5}.commit-plan-summary>span{display:inline-block;color:var(--warning);margin:10px 0}.commit-plan-summary>p{color:var(--muted)}.commit-plan-summary article{padding:11px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);margin-top:8px}.commit-group-title{display:flex;justify-content:space-between;align-items:center}.commit-group-title i{font-style:normal;color:var(--muted);font-size:9px}.commit-group-title i.committed{color:var(--success)}.commit-plan-summary textarea{width:100%;min-height:98px;margin:10px 0 7px;border:1px solid var(--line);border-radius:7px;background:var(--surface-2);padding:9px 10px;resize:vertical;line-height:1.6;font-family:inherit;outline:0}.commit-plan-summary article small{color:var(--muted)}.commit-message-help{display:block;margin-bottom:6px;line-height:1.5}.commit-plan-summary details{margin:8px 0;color:var(--muted)}.commit-plan-summary details p{margin:4px 0;font-family:monospace;font-size:9px;overflow-wrap:anywhere}.git-history article{display:grid;grid-template-columns:64px minmax(0,1fr) auto;align-items:center;gap:9px;padding:10px 0;border-bottom:1px solid var(--line)}.git-history article>code{color:var(--primary)}.git-history article>span{min-width:0;display:flex;flex-direction:column;gap:4px}.git-history b{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.git-history small,.git-empty{color:var(--muted);font-size:9px}.danger-text{color:var(--danger)}.git-output pre{max-height:180px;margin:0;overflow:auto;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);padding:11px;white-space:pre-wrap;overflow-wrap:anywhere;color:var(--muted);font:10px/1.6 monospace}
.asset-row-actions{gap:5px}.asset-row-button{height:30px;min-width:48px;border:1px solid var(--line);border-radius:7px;background:var(--surface-2);color:var(--muted);padding:0 9px;font-weight:700;white-space:nowrap}.asset-row-button:hover{border-color:color-mix(in srgb,var(--primary) 42%,var(--line));color:var(--primary)}.asset-row-button.launch{min-width:68px;background:var(--primary-soft);border-color:color-mix(in srgb,var(--primary) 42%,var(--line));color:var(--primary)}.asset-row-button.launch.stop{border-color:color-mix(in srgb,var(--danger) 48%,var(--line));background:color-mix(in srgb,var(--danger) 11%,var(--surface));color:var(--danger)}.asset-row-button.git{color:var(--primary)}.asset-row-button:disabled{opacity:.45;cursor:not-allowed}
.drawer-project-title{min-width:0;flex:1}.drawer-project-title p{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.drawer-header-actions{display:flex;align-items:flex-start;justify-content:flex-end;gap:6px;margin-left:14px}.drawer-action-button{height:36px;min-width:58px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);color:var(--muted);padding:0 10px;font-weight:700;white-space:nowrap}.drawer-action-button:hover,.drawer-action-button.active{border-color:color-mix(in srgb,var(--primary) 45%,var(--line));color:var(--primary);background:var(--primary-soft)}.drawer-action-button.primary{min-width:70px;border-color:color-mix(in srgb,var(--primary) 48%,var(--line));background:var(--primary);color:white}.drawer-action-button.primary.stop{border-color:color-mix(in srgb,var(--danger) 48%,var(--line));background:color-mix(in srgb,var(--danger) 11%,var(--surface));color:var(--danger)}.drawer-action-button.close{min-width:36px;width:36px;padding:0;font-size:17px}
.asset-overview>header{display:flex;align-items:center;justify-content:space-between;gap:12px}.asset-overview>header h3{margin:0}.asset-overview-actions{display:flex;gap:8px}.auto-launch-tip{margin:10px 0 13px;padding:9px 11px;border-radius:7px;background:var(--primary-soft);color:var(--muted);font-size:10px;line-height:1.5}.asset-commands{padding:0;border-bottom:1px solid var(--line)}.asset-commands>summary{display:flex;align-items:center;justify-content:space-between;gap:12px;padding:14px 20px;cursor:pointer;font-weight:800}.asset-commands>summary small{font-weight:400;color:var(--muted);white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.asset-commands[open]{padding:0 20px 16px}.asset-commands[open]>summary{margin:0 -20px 13px}.asset-form h3{margin-bottom:14px}.asset-form-advanced{margin:4px 0 14px;border:1px solid var(--line);border-radius:8px;padding:0 12px}.asset-form-advanced>summary{cursor:pointer;padding:11px 0;color:var(--primary);font-weight:700}.asset-form-advanced[open]{padding-bottom:4px}
.git-remote-actions{display:grid;grid-template-columns:repeat(4,max-content);align-items:center;gap:8px}.git-remote-actions .remote-update-available{border-color:var(--warning);background:color-mix(in srgb,var(--warning) 14%,var(--surface));color:var(--warning);box-shadow:0 0 0 1px color-mix(in srgb,var(--warning) 20%,transparent)}.git-action-help{display:block;margin-top:9px;color:var(--muted);line-height:1.5}.remote-refresh-note{display:block;margin-top:6px;color:var(--muted);line-height:1.5}.remote-refresh-note.updates{color:var(--warning)}.git-changed-files{margin-top:12px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);overflow:hidden}.git-changed-files summary{cursor:pointer;padding:10px 12px;color:var(--primary);font-weight:700}.git-stage-select-all{display:flex;align-items:center;gap:8px;padding:8px 12px;border-top:1px solid var(--line);color:var(--muted)}.git-stage-select-all span{margin-left:auto;font-size:9px}.git-changed-files input{width:15px;height:15px;accent-color:var(--primary)}.git-changed-files article{display:grid;grid-template-columns:18px 64px minmax(0,1fr) auto;align-items:center;gap:9px;padding:8px 12px;border-top:1px solid var(--line)}.git-stage-check{color:var(--success);text-align:center}.git-changed-files i{font-style:normal;color:var(--muted);font-size:9px}.git-changed-files code{min-width:0;overflow-wrap:anywhere}.git-changed-files em{font-style:normal;color:var(--muted);font-size:9px;white-space:nowrap}.git-changed-files em.staged{color:var(--success)}
.asset-metrics .running b{color:var(--success)}.project-view-tabs{height:48px;display:flex;align-items:center;gap:6px;padding:7px 14px;border-bottom:1px solid var(--line)}.project-view-tabs button{height:32px;border:1px solid transparent;border-radius:8px;background:transparent;color:var(--muted);padding:0 14px}.project-view-tabs button.active{border-color:color-mix(in srgb,var(--primary) 30%,var(--line));background:var(--primary-soft);color:var(--primary);font-weight:800}.asset-table{min-width:1320px}.branch-name{max-width:170px!important;color:var(--primary)}.status-stack{display:flex;align-items:flex-start;flex-direction:column;gap:5px}.status-stack small{margin:0!important}.runtime-pill,.pending-pill{display:inline-flex;align-items:center;width:max-content;padding:5px 8px;border-radius:12px;font-style:normal;font-size:9px}.runtime-pill.starting{color:var(--warning);background:color-mix(in srgb,var(--warning) 12%,transparent)}.runtime-pill.running{color:var(--success);background:color-mix(in srgb,var(--success) 12%,transparent)}.runtime-pill.failed{color:var(--danger);background:color-mix(in srgb,var(--danger) 12%,transparent)}.runtime-pill.stopped,.runtime-pill.idle{color:var(--muted);background:var(--surface-2)}.runtime-pill.large{font-size:10px;padding:7px 10px}.pending-pill.pending-high{color:var(--danger);background:color-mix(in srgb,var(--danger) 12%,transparent)}.pending-pill.pending-medium{color:var(--warning);background:color-mix(in srgb,var(--warning) 12%,transparent)}.pending-pill.pending-low{color:var(--muted);background:var(--surface-2)}.pending-pill.pending-none{color:var(--success);background:color-mix(in srgb,var(--success) 10%,transparent)}
.scan-settings-backdrop{z-index:240;align-items:center;justify-content:center;padding:30px}.scan-settings-dialog{width:min(720px,calc(100vw - 80px));max-height:calc(100vh - 70px);overflow:auto}.scan-settings-dialog>header{padding:17px 20px;border-bottom:1px solid var(--line);display:flex;align-items:flex-start;justify-content:space-between}.scan-settings-dialog h2{margin:0 0 5px}.scan-settings-dialog header p{margin:0;color:var(--muted)}.scan-settings-body{display:grid;gap:17px;padding:18px 20px}.scan-settings-body>label{display:grid;grid-template-columns:130px 90px 1fr;align-items:center;gap:10px}.scan-settings-body input{height:38px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);padding:0 10px}.scan-settings-body small{color:var(--muted);line-height:1.5}.scan-settings-body section>b{display:block;margin-bottom:9px}.scan-chip-list{display:flex;flex-wrap:wrap;gap:7px;min-height:30px}.scan-chip-list>span{display:flex;align-items:center;gap:6px;max-width:100%;padding:6px 8px;border-radius:7px;background:var(--surface-2)}.scan-chip-list code{overflow-wrap:anywhere}.scan-chip-list button{border:0;background:transparent;color:var(--muted)}.scan-add-row{display:grid;grid-template-columns:1fr auto;gap:8px;margin-top:9px}.scan-history{padding:12px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2)}.scan-history p{margin:6px 0;color:var(--muted)}.scan-history ul{max-height:100px;overflow:auto;margin:7px 0 0;padding-left:19px;color:var(--danger)}.scan-settings-dialog>footer{display:flex;justify-content:flex-end;gap:8px;padding:0 20px 20px}
.runtime-console{padding:18px 20px;border-bottom:1px solid var(--line)}.runtime-console>header{display:flex;justify-content:space-between;gap:12px}.runtime-console h3{margin:0 0 5px}.runtime-console header p{margin:0;color:var(--muted)}.runtime-metrics{display:grid;grid-template-columns:repeat(3,1fr);gap:8px;margin-top:13px}.runtime-metrics article{padding:10px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2)}.runtime-metrics small{display:block;color:var(--muted);font-size:9px}.runtime-metrics b{display:block;margin-top:6px}.runtime-url{display:flex;align-items:center;gap:8px;margin-top:10px;padding:9px 10px;border-radius:8px;background:var(--primary-soft)}.runtime-url code{min-width:0;flex:1;overflow-wrap:anywhere;color:var(--primary)}.runtime-error{padding:9px 10px;border-radius:8px;background:color-mix(in srgb,var(--danger) 10%,transparent);color:var(--danger)}.runtime-log{margin-top:11px;border:1px solid var(--line);border-radius:8px;overflow:hidden}.runtime-log>header{display:flex;align-items:center;justify-content:space-between;padding:8px 10px;background:var(--surface-2)}.runtime-log header>div{display:flex;align-items:center;gap:8px;min-width:0}.runtime-log header small{max-width:280px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;color:var(--muted)}.runtime-log pre{max-height:210px;min-height:70px;margin:0;padding:10px;overflow:auto;white-space:pre-wrap;overflow-wrap:anywhere;color:var(--muted);font:10px/1.55 monospace}.continue-card{display:flex;align-items:center;gap:14px;margin:14px 20px 0;padding:13px 15px;border:1px solid color-mix(in srgb,var(--primary) 30%,var(--line));border-radius:9px;background:var(--primary-soft)}.continue-card>div{min-width:0;flex:1}.continue-card small,.continue-card b{display:block}.continue-card small{color:var(--muted)}.continue-card b{margin-top:5px}.continue-card p{margin:5px 0 0;color:var(--muted)}.drawer-action-button.continue{border-color:color-mix(in srgb,var(--primary) 45%,var(--line));color:var(--primary)}
.association-center{padding:18px 20px}.association-center>header{display:flex;justify-content:space-between;gap:12px;margin-bottom:14px}.association-center h3{margin:0 0 5px}.association-center header p{margin:0;color:var(--muted)}.association-center>header>span{color:var(--muted)}.association-grid{display:grid;grid-template-columns:1fr 1fr;gap:12px}.association-grid>section{min-width:0;border:1px solid var(--line);border-radius:9px;overflow:hidden}.association-grid>section>header{display:flex;justify-content:space-between;padding:10px 12px;background:var(--surface-2)}.association-grid>section>header span{color:var(--muted)}.association-grid button{width:100%;display:grid;grid-template-columns:52px minmax(0,1fr) auto;align-items:center;gap:9px;border:0;border-top:1px solid var(--line);background:var(--surface);color:inherit;padding:10px 12px;text-align:left}.association-grid button:not(:disabled):hover{background:var(--primary-soft)}.association-grid button:disabled{opacity:1;cursor:default}.association-grid button>i{font-style:normal;font-size:8px;color:var(--primary)}.association-grid button>span{min-width:0}.association-grid button b,.association-grid button small{display:block;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.association-grid button small,.association-grid button em{margin-top:4px;color:var(--muted);font-size:9px}.association-grid button em{font-style:normal}.association-grid>.empty-state{grid-column:1/-1}
.pull-conflict-backdrop{z-index:260;align-items:center;justify-content:center;padding:30px}.pull-conflict-dialog{width:min(720px,calc(100vw - 32px));max-height:calc(100vh - 60px);overflow:auto}.pull-conflict-dialog>header{display:flex;align-items:flex-start;justify-content:space-between;gap:12px;padding:18px 20px;border-bottom:1px solid var(--line)}.pull-conflict-dialog h2{margin:0 0 6px}.pull-conflict-dialog header p{margin:0;color:var(--muted)}.pull-conflict-body{display:grid;gap:14px;padding:18px 20px}.pull-conflict-body ul{max-height:130px;margin:9px 0 0;padding:9px 10px 9px 28px;overflow:auto;border:1px solid var(--line);border-radius:8px;background:var(--surface-2)}.pull-conflict-body li{margin:5px 0}.pull-conflict-body code{overflow-wrap:anywhere}.pull-conflict-options{display:grid;grid-template-columns:1fr 1fr;gap:9px}.pull-conflict-options button{display:flex;min-width:0;flex-direction:column;gap:6px;border:1px solid var(--line);border-radius:9px;background:var(--surface-2);color:inherit;padding:12px;text-align:left}.pull-conflict-options button.ai{grid-column:1/-1;border-color:color-mix(in srgb,var(--primary) 45%,var(--line));background:var(--primary-soft)}.pull-conflict-options button:not(:disabled):hover{border-color:var(--primary)}.pull-conflict-options span{color:var(--muted);line-height:1.5}.pull-conflict-options button:disabled{opacity:.5}.pull-conflict-warning{margin:0;padding:9px 10px;border-radius:8px;background:color-mix(in srgb,var(--warning) 12%,transparent);color:var(--warning);line-height:1.5}.pull-conflict-progress{margin:0;color:var(--primary)}.pull-conflict-dialog>footer{display:flex;justify-content:flex-end;padding:0 20px 18px}
@media(max-width:1350px){.asset-drawer{width:700px}.git-metrics{grid-template-columns:repeat(2,1fr)}.git-remote-actions{grid-template-columns:repeat(2,max-content)}.git-credential-panel>div{grid-template-columns:1fr 1fr}.git-credential-panel>div .button{width:100%}}
.asset-drawer{width:min(920px,calc(100vw - 40px))}.drawer-header-actions{flex-wrap:wrap}.continue-card>div>p{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
@media(max-width:900px){.asset-metrics{grid-template-columns:repeat(2,1fr)}.asset-drawer{width:100vw}.association-grid,.pull-conflict-options{grid-template-columns:1fr}.pull-conflict-options button.ai{grid-column:auto}.runtime-metrics{grid-template-columns:1fr}.scan-settings-dialog{width:calc(100vw - 24px)}}
.asset-table-wrap{overflow-x:hidden;overflow-y:auto}.asset-table{width:100%;min-width:0;max-width:100%;table-layout:fixed}.asset-table th:nth-child(1){width:52px}.asset-table th:nth-child(2){width:20%}.asset-table th:nth-child(3){width:16%}.asset-table th:nth-child(4){width:13%}.asset-table th:nth-child(5){width:15%}.asset-table th:nth-child(6){width:24%}.asset-table th:nth-child(7){width:10%}.asset-table td{overflow:hidden}.asset-table td b,.asset-table td small{max-width:100%}.asset-table th,.asset-table td{padding:9px 8px}.asset-row-actions{display:grid;grid-template-columns:1fr;gap:5px;white-space:normal}.asset-row-actions .asset-row-button{width:100%;min-width:0;padding:0 4px}.work-state,.runtime-cell{display:flex;align-items:flex-start;flex-direction:column;gap:6px;min-width:0}.work-state>*{max-width:100%;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.clean-state{margin:0!important;color:var(--muted)}.branch-sync{margin-top:5px!important;color:var(--muted)}.branch-sync.behind{color:var(--warning)}.runtime-address-button{display:block;max-width:100%;border:0;background:transparent;color:var(--primary);padding:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;text-align:left;text-decoration:underline;text-underline-offset:2px;cursor:pointer}.runtime-address-empty{margin:0!important;color:var(--muted)}
.commit-plan-actions{display:flex;align-items:center;justify-content:flex-end;gap:8px;flex-wrap:wrap}.commit-grouping-switch{height:36px;display:flex;align-items:center;padding:3px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2)}.commit-grouping-switch button{height:28px;border:0;border-radius:6px;background:transparent;color:var(--muted);padding:0 10px;white-space:nowrap}.commit-grouping-switch button.active{background:var(--primary-soft);color:var(--primary);font-weight:800}.commit-plan-meta{display:flex;align-items:center;gap:7px;flex-wrap:wrap;margin:10px 0}.commit-plan-meta>span{margin:0;padding:5px 8px;border-radius:6px;background:var(--primary-soft);color:var(--primary);font-size:9px}.commit-plan-meta>span:first-child,.commit-plan-meta>span.fallback{background:color-mix(in srgb,var(--warning) 12%,transparent);color:var(--warning)}.commit-generation-warning{padding:8px 10px;border-radius:7px;background:color-mix(in srgb,var(--warning) 10%,transparent);color:var(--warning)!important}.commit-excluded-files{margin:9px 0;padding:9px 11px;border:1px solid color-mix(in srgb,var(--warning) 35%,var(--line));border-radius:8px;color:var(--warning)}.commit-excluded-files summary{cursor:pointer}.commit-excluded-files p{margin:5px 0 0;color:var(--muted);font:9px/1.5 monospace;overflow-wrap:anywhere}
@media(max-width:900px){.commit-plan>header{flex-direction:column}.commit-plan-actions{width:100%;justify-content:flex-start}.commit-grouping-switch{max-width:100%}.commit-grouping-switch button{padding:0 7px}}
.git-changed-files article{grid-template-columns:18px 64px minmax(0,1fr) auto auto}.git-file-actions{display:flex;align-items:center;justify-content:flex-end;gap:7px;white-space:nowrap}.git-file-actions .text-button{font-size:9px}.git-diff-preview{margin-top:12px;border:1px solid var(--line);border-radius:9px;overflow:hidden;background:var(--surface-2)}.git-diff-preview>header{display:flex;align-items:flex-start;justify-content:space-between;gap:10px;padding:11px 12px;border-bottom:1px solid var(--line)}.git-diff-preview h4{margin:0 0 5px}.git-diff-preview header code{color:var(--muted);overflow-wrap:anywhere}.diff-warning{margin:10px 12px 0;color:var(--warning)}.diff-section>b{display:block;padding:9px 12px;color:var(--muted)}.diff-code{max-height:360px;overflow-y:auto;border-top:1px solid var(--line);background:#0b0e14;font:10px/1.55 monospace}.diff-code span{display:block;min-height:16px;padding:0 10px;white-space:pre-wrap;overflow-wrap:anywhere;color:#aeb6c5}.diff-code span.addition{background:rgba(46,160,67,.16);color:#7ee787}.diff-code span.deletion{background:rgba(248,81,73,.16);color:#ff7b72}.diff-code span.hunk{background:rgba(88,166,255,.12);color:#79c0ff}.diff-code span.meta{color:#8b949e}.post-commit-guide{display:flex;align-items:center;justify-content:space-between;gap:14px;padding:15px 20px;border-bottom:1px solid var(--line);background:color-mix(in srgb,var(--success) 9%,transparent)}.post-commit-guide>div:first-child{min-width:0}.post-commit-guide small,.post-commit-guide b,.post-commit-guide p{display:block}.post-commit-guide small{color:var(--success)}.post-commit-guide b{margin-top:5px}.post-commit-guide code{display:inline-block;margin-top:6px;color:var(--primary)}.post-commit-guide p{margin:5px 0 0;color:var(--muted)}.post-commit-guide>div:last-child{display:flex;gap:8px;flex-shrink:0}.git-safety-panel,.git-operation-log{padding:18px 20px;border-bottom:1px solid var(--line)}.git-safety-panel>header,.git-operation-log>header{display:flex;align-items:flex-start;justify-content:space-between;gap:12px}.git-safety-panel header p,.git-operation-log header p{margin:4px 0 0;color:var(--muted)}.git-safety-panel>div{display:flex;gap:8px;flex-wrap:wrap;margin:12px 0 8px}.git-safety-panel>small{color:var(--muted)}.git-operation-log article{margin-top:9px;padding:10px 11px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2)}.git-operation-log article.success{border-left:3px solid var(--success)}.git-operation-log article.error{border-left:3px solid var(--danger)}.git-operation-log article>div{display:flex;align-items:center;gap:8px}.git-operation-log article i{font-style:normal;color:var(--success);font-size:9px}.git-operation-log article.error i{color:var(--danger)}.git-operation-log article time{margin-left:auto;color:var(--muted);font-size:9px}.git-operation-log article p{margin:7px 0 0;color:var(--muted)}.git-operation-log details{margin-top:8px}.git-operation-log summary{cursor:pointer;color:var(--primary)}.git-operation-log pre{max-height:220px;margin:8px 0 0;padding:9px;overflow:auto;border-radius:7px;background:#0b0e14;color:#aeb6c5;white-space:pre-wrap;overflow-wrap:anywhere;font:10px/1.55 monospace}
@media(max-width:900px){.git-changed-files article{grid-template-columns:18px 58px minmax(0,1fr)}.git-changed-files article>em{grid-column:2}.git-file-actions{grid-column:3;justify-content:flex-start;flex-wrap:wrap}.post-commit-guide{align-items:flex-start;flex-direction:column}.post-commit-guide>div:last-child{width:100%}}
</style>
