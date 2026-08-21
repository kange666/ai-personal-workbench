<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useRouter } from "vue-router";
import {
  clearGitDefaultCredential,
  commitGitPlanGroup,
  fetchGitRepository,
  generateCommitPlan,
  getGitRepositoryStatus,
  getRepositoryAssetDetails,
  isTauriRuntime,
  listRepositoryAssets,
  mergeGitRepositoryBranch,
  pullGitRepository,
  revertGitRepositoryCommit,
  saveGitDefaultCredential,
  saveRepositoryAsset,
  scanGitRepositories,
  setRepositoryHidden,
  setRepositoryPinned,
  switchGitRepositoryBranch,
  type GitOperationResult,
  type GitRepositoryStatus,
  type RepositoryAsset,
  type RepositoryAssetDetails,
  type RepositoryAssetUpdate,
} from "../services/backend";

const router = useRouter();
const assets = ref<RepositoryAsset[]>([]);
const selected = ref<RepositoryAsset | null>(null);
const details = ref<RepositoryAssetDetails>({ conversations: [], commits: [] });
const gitStatus = ref<GitRepositoryStatus | null>(null);
const showHiddenList = ref(false);
const activeTab = ref<"overview" | "git">("overview");
const query = ref("");
const health = ref("全部状态");
const changes = ref("全部工作区");
const loading = ref(false);
const editing = ref(false);
const message = ref("");
const error = ref("");
const gitOutput = ref("");
const switchBranch = ref("");
const mergeBranch = ref("");
const credentialUsername = ref("lzsk");
const credentialSecret = ref("");
const commitMessages = reactive<Record<string, string>>({});
const form = reactive<RepositoryAssetUpdate>({
  path: "", category: "待确认", purpose: "", technologyStack: "", mainModules: "",
  installCommand: "", startCommand: "", testCommand: "", buildCommand: "", commandSource: "",
});

const visibleAssets = computed(() => assets.value.filter((item) => !item.isHidden));
const hiddenAssets = computed(() => assets.value.filter((item) => item.isHidden));
const filtered = computed(() => {
  const keyword = query.value.trim().toLowerCase();
  return visibleAssets.value.filter((item) => {
    if (health.value !== "全部状态" && item.healthLevel !== health.value) return false;
    if (changes.value === "有未提交修改" && !item.hasUncommittedChanges) return false;
    if (changes.value === "工作区干净" && item.hasUncommittedChanges) return false;
    return !keyword || [item.name, item.path, item.purpose, item.technologyStack, item.category].some((value) => value.toLowerCase().includes(keyword));
  }).sort((left, right) => Number(right.isPinned) - Number(left.isPinned) || Date.parse(right.updatedAt) - Date.parse(left.updatedAt) || left.name.localeCompare(right.name, "zh-CN"));
});
const dirtyCount = computed(() => visibleAssets.value.filter((item) => item.hasUncommittedChanges).length);
const confirmedCount = computed(() => visibleAssets.value.filter((item) => item.manuallyConfirmed).length);
const failedCount = computed(() => visibleAssets.value.filter((item) => item.healthLevel === "失败").length);
const mergeOptions = computed(() => gitStatus.value?.branches.filter((branch) => branch !== gitStatus.value?.currentBranch) ?? []);

function healthClass(value: string) { return value === "健康" ? "healthy" : value === "警告" ? "warning" : value === "失败" ? "failed" : "unknown"; }
function fillForm(item: RepositoryAsset) { Object.assign(form, { path: item.path, category: item.category, purpose: item.purpose, technologyStack: item.technologyStack, mainModules: item.mainModules, installCommand: item.installCommand, startCommand: item.startCommand, testCommand: item.testCommand, buildCommand: item.buildCommand, commandSource: item.commandSource }); }
function syncCommitMessages() { for (const key of Object.keys(commitMessages)) delete commitMessages[key]; for (const group of details.value.commitPlan?.groups ?? []) commitMessages[group.id] = group.commitMessage; }

async function load() {
  if (!isTauriRuntime()) return;
  loading.value = true; error.value = "";
  try { assets.value = await listRepositoryAssets(); } catch (cause) { error.value = String(cause); } finally { loading.value = false; }
}
async function loadGitStatus(path: string) {
  gitStatus.value = await getGitRepositoryStatus(path);
  switchBranch.value = gitStatus.value.currentBranch;
  mergeBranch.value = gitStatus.value.branches.find((branch) => branch !== gitStatus.value?.currentBranch) ?? "";
  credentialUsername.value = gitStatus.value.credential.username || "lzsk";
}
async function openAsset(item: RepositoryAsset, tab: "overview" | "git" = "overview") {
  selected.value = item; activeTab.value = tab; editing.value = false; fillForm(item); error.value = "";
  try {
    [details.value] = await Promise.all([getRepositoryAssetDetails(item.path), loadGitStatus(item.path)]);
    syncCommitMessages();
  } catch (cause) { error.value = String(cause); details.value = { conversations: [], commits: [] }; gitStatus.value = null; }
}
async function refreshSelected() {
  const path = selected.value?.path; await load(); if (!path) return;
  const refreshed = assets.value.find((item) => item.path === path); if (refreshed) await openAsset(refreshed, activeTab.value);
}
async function scan() {
  loading.value = true; error.value = ""; message.value = "";
  try { const result = await scanGitRepositories(); message.value = `已扫描 ${result.repositoriesFound} 个仓库，${result.errors} 个项目需要人工确认。`; await load(); }
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
async function planCommit() {
  if (!selected.value) return; loading.value = true; error.value = "";
  try { details.value.commitPlan = await generateCommitPlan(selected.value.path); syncCommitMessages(); message.value = "提交建议已生成，Git 暂存区没有变化。"; }
  catch (cause) { error.value = String(cause); } finally { loading.value = false; }
}
async function runGitAction(action: () => Promise<GitOperationResult>, confirmation?: string) {
  if (confirmation && !window.confirm(confirmation)) return;
  loading.value = true; error.value = ""; message.value = "";
  try { const result = await action(); message.value = result.message; gitOutput.value = result.output; await refreshSelected(); }
  catch (cause) { error.value = String(cause); } finally { loading.value = false; }
}
async function commitGroup(groupId: string, files: string[]) {
  if (!selected.value) return; const commitMessage = commitMessages[groupId]?.trim() ?? "";
  await runGitAction(() => commitGitPlanGroup(selected.value!.path, groupId, commitMessage), `确认提交这 ${files.length} 个文件吗？\n\n${commitMessage}\n\n本次只提交该建议组，不会自动推送。`);
}
async function saveCredential() {
  loading.value = true; error.value = "";
  try { await saveGitDefaultCredential(credentialUsername.value, credentialSecret.value); credentialSecret.value = ""; message.value = "默认 Git 凭据已保存到 Windows 凭据库。"; if (selected.value) await loadGitStatus(selected.value.path); }
  catch (cause) { error.value = String(cause); } finally { loading.value = false; }
}
async function clearCredential() {
  if (!window.confirm("确定删除工作台保存的默认 Git 凭据吗？项目自身的 Git 配置不会被修改。")) return;
  await clearGitDefaultCredential(); credentialSecret.value = ""; credentialUsername.value = "lzsk"; message.value = "工作台默认 Git 凭据已删除。"; if (selected.value) await loadGitStatus(selected.value.path);
}
async function save() {
  loading.value = true; error.value = "";
  try { await saveRepositoryAsset({ ...form }); message.value = "人工修正已保存，后续扫描不会覆盖这些说明。"; editing.value = false; await refreshSelected(); }
  catch (cause) { error.value = String(cause); } finally { loading.value = false; }
}
function exportText(format: "markdown" | "csv") {
  if (format === "csv") {
    const quote = (value: string | number | boolean) => `"${String(value).replaceAll('"', '""')}"`;
    return [["置顶", "项目", "路径", "分类", "用途", "技术栈", "分支", "健康状态", "未提交修改", "Codex任务", "提交记录"].map(quote).join(","), ...filtered.value.map((item) => [item.isPinned ? "是" : "否", item.name, item.path, item.category, item.purpose, item.technologyStack, item.defaultBranch, item.healthLevel, item.hasUncommittedChanges ? "是" : "否", item.conversationCount, item.commitCount].map(quote).join(","))].join("\n");
  }
  return ["# 项目资产清单", "", `共 ${filtered.value.length} 个项目`, "", ...filtered.value.flatMap((item) => [`## ${item.isPinned ? "📌 " : ""}${item.name}`, `- 路径：${item.path}`, `- 分类：${item.category}`, `- 用途：${item.purpose || "待确认"}`, `- 技术栈：${item.technologyStack || "待确认"}`, `- 健康：${item.healthLevel}${item.hasUncommittedChanges ? "（有未提交修改）" : ""}`, ""])].join("\n");
}
async function copyExport(format: "markdown" | "csv") { await navigator.clipboard.writeText(exportText(format)); message.value = `${format === "csv" ? "CSV" : "Markdown"} 清单已复制，可直接保存为文件。`; }
onMounted(load);
</script>

<template>
  <div class="view projects-view">
    <header class="page-header"><div><h1>项目资产</h1><p>统一查看本机 Git 仓库、关联 Codex 任务、健康状态和运行说明</p></div><div><button class="button secondary" @click="showHiddenList = true">隐藏列表 {{ hiddenAssets.length }}</button><button class="button secondary" @click="copyExport('csv')">导出 CSV</button><button class="button secondary" @click="copyExport('markdown')">导出 Markdown</button><button class="button primary" :disabled="loading" @click="scan">{{ loading ? "处理中…" : "↻ 重新扫描" }}</button></div></header>
    <div v-if="error || message" class="scan-message" :class="{ error: Boolean(error) }">{{ error || message }}</div>
    <section class="asset-metrics"><article class="panel"><small>显示中的项目</small><b>{{ visibleAssets.length }}</b><span>另有 {{ hiddenAssets.length }} 个项目已隐藏</span></article><article class="panel warning"><small>有未提交修改</small><b>{{ dirtyCount }}</b><span>仅提示，不等同于项目故障</span></article><article class="panel"><small>人工确认</small><b>{{ confirmedCount }}</b><span>人工说明优先保留</span></article><article class="panel failed"><small>扫描失败</small><b>{{ failedCount }}</b><span>需要查看失败原因</span></article></section>
    <section class="panel asset-workspace">
      <div class="asset-toolbar"><label>⌕<input v-model="query" placeholder="搜索项目、路径、用途或技术栈"></label><select v-model="health"><option>全部状态</option><option>健康</option><option>警告</option><option>失败</option><option>未验证</option></select><select v-model="changes"><option>全部工作区</option><option>有未提交修改</option><option>工作区干净</option></select><span>{{ filtered.length }} / {{ visibleAssets.length }}</span></div>
      <div class="asset-table-wrap"><table class="asset-table"><thead><tr><th class="pin-column">置顶</th><th>项目</th><th>用途与分类</th><th>技术栈</th><th>关联数据</th><th>健康</th><th>工作区</th><th></th></tr></thead><tbody><tr v-for="item in filtered" :key="item.path" :class="{ pinned: item.isPinned }" @click="openAsset(item)"><td><button class="pin-button" :class="{ active: item.isPinned }" :title="item.isPinned ? '取消置顶' : '置顶项目'" @click.stop="togglePin(item)">{{ item.isPinned ? "★" : "☆" }}</button></td><td><b>{{ item.name }}</b><small>{{ item.path }}</small></td><td><span>{{ item.category }}</span><small>{{ item.purpose || "AI 推断待确认" }}</small></td><td>{{ item.technologyStack || "待确认" }}</td><td><span>{{ item.conversationCount }} 个 Codex 任务</span><small>{{ item.commitCount }} 条最近提交</small></td><td><i class="health-pill" :class="healthClass(item.healthLevel)">{{ item.healthLevel }}</i></td><td><i v-if="item.hasUncommittedChanges" class="dirty-pill">有修改</i><span v-else class="clean-text">干净</span></td><td><div class="asset-row-actions"><button class="text-button muted-action" @click.stop="toggleHidden(item, true)">隐藏</button><button class="text-button" @click.stop="openAsset(item, 'git')">Git →</button></div></td></tr></tbody></table><div v-if="!filtered.length" class="empty-state"><b>没有符合条件的项目</b><p>请调整筛选条件或重新扫描。</p></div></div>
    </section>
    <div v-if="showHiddenList" class="activity-backdrop hidden-project-backdrop" @click.self="showHiddenList = false"><section class="hidden-project-dialog panel"><header><div><h2>隐藏项目</h2><p>隐藏只影响项目资产列表，不会删除仓库、报告或历史数据。</p></div><button class="icon-button" @click="showHiddenList = false">×</button></header><div class="hidden-project-list"><article v-for="item in hiddenAssets" :key="item.path"><div><b>{{ item.name }}</b><small>{{ item.path }}</small></div><button class="button secondary small" @click="toggleHidden(item, false)">恢复显示</button></article><div v-if="!hiddenAssets.length" class="empty-state"><b>没有隐藏项目</b><p>从项目列表点击“隐藏”后，会显示在这里。</p></div></div></section></div>
    <div v-if="selected" class="activity-backdrop" @click.self="selected = null"><aside class="asset-drawer panel">
      <header><div><small>PROJECT ASSET</small><h2>{{ selected.name }}</h2><p>{{ selected.path }}</p></div><div class="drawer-header-actions"><button class="button secondary small" @click="toggleHidden(selected, true)">隐藏</button><button class="pin-button" :class="{ active: selected.isPinned }" @click="togglePin(selected)">{{ selected.isPinned ? "★ 已置顶" : "☆ 置顶" }}</button><button class="icon-button" @click="selected = null">×</button></div></header>
      <div class="asset-status-strip"><span class="health-pill" :class="healthClass(selected.healthLevel)">{{ selected.healthLevel }}</span><span>{{ gitStatus?.currentBranch || selected.defaultBranch || "无分支" }}</span><span>{{ gitStatus?.hasUncommittedChanges ? "有未提交修改" : "工作区干净" }}</span><span>{{ selected.manuallyConfirmed ? "人工已确认" : "AI 推断待确认" }}</span></div>
      <nav class="asset-tabs"><button :class="{ active: activeTab === 'overview' }" @click="activeTab = 'overview'">项目说明</button><button :class="{ active: activeTab === 'git' }" @click="activeTab = 'git'">Git 操作</button></nav>
      <template v-if="activeTab === 'overview'">
        <template v-if="!editing"><section class="asset-overview"><h3>项目说明</h3><dl><div><dt>分类</dt><dd>{{ selected.category }}</dd></div><div><dt>用途</dt><dd>{{ selected.purpose || "尚未确认项目用途" }}</dd></div><div><dt>技术栈</dt><dd>{{ selected.technologyStack || "尚未确认技术栈" }}</dd></div><div><dt>主要模块</dt><dd>{{ selected.mainModules || "尚未整理主要模块" }}</dd></div></dl><button class="button primary" @click="editing = true">编辑并人工确认</button></section><section class="asset-commands"><h3>运行与验证命令</h3><div v-for="[label, value] in [['安装', selected.installCommand], ['启动', selected.startCommand], ['测试', selected.testCommand], ['构建', selected.buildCommand]]" :key="label"><span>{{ label }}</span><code>{{ value || "待确认，不自动执行" }}</code></div><small>来源：{{ selected.commandSource || "尚未确认" }}。工作台不会从 README 任意执行命令。</small></section></template>
        <form v-else class="asset-form" @submit.prevent="save"><label>项目分类<input v-model="form.category" placeholder="例如：业务系统 / 工具 / 视频工程"></label><label>项目用途<textarea v-model="form.purpose" rows="3" placeholder="一句话说明这个项目解决什么问题"></textarea></label><label>技术栈<input v-model="form.technologyStack" placeholder="例如：Vue 3 / Vite / Element Plus"></label><label>主要模块<textarea v-model="form.mainModules" rows="2" placeholder="用逗号分隔主要功能模块"></textarea></label><div class="form-grid"><label>安装命令<input v-model="form.installCommand"></label><label>启动命令<input v-model="form.startCommand"></label><label>测试命令<input v-model="form.testCommand"></label><label>构建命令<input v-model="form.buildCommand"></label></div><label>命令来源<input v-model="form.commandSource" placeholder="例如：package.json scripts（已核对）"></label><footer><button type="button" class="button secondary" @click="editing = false">取消</button><button class="button primary" :disabled="loading">保存人工结果</button></footer></form>
        <section class="asset-related"><div><h3>关联 Codex 任务</h3><button v-for="item in details.conversations" :key="item.id" @click="router.push(`/tokens?conversation=${item.id}`)"><span><b>{{ item.title }}</b><small>{{ item.updatedAt.slice(0, 10) }} · {{ item.archived ? "归档任务" : "普通任务" }}</small></span><em>查看 →</em></button><p v-if="!details.conversations.length">暂无直接关联任务。</p></div><div><h3>最近 Git 提交</h3><article v-for="item in details.commits.slice(0, 8)" :key="item.hash"><code>{{ item.hash.slice(0, 7) }}</code><span><b>{{ item.subject }}</b><small>{{ item.committedAt.slice(0, 10) }}</small></span></article><p v-if="!details.commits.length">暂无可读取提交。</p></div></section>
      </template>
      <template v-else>
        <section v-if="gitStatus" class="git-dashboard"><header><div><h3>仓库状态</h3><p>{{ gitStatus.remoteUrl || "未配置 origin 远程仓库" }}</p></div><button class="button secondary small" :disabled="loading" @click="loadGitStatus(selected.path)">刷新状态</button></header><div class="git-metrics"><article><small>当前分支</small><b>{{ gitStatus.currentBranch }}</b></article><article><small>上游分支</small><b>{{ gitStatus.upstream || "未关联" }}</b></article><article><small>领先 / 落后</small><b>{{ gitStatus.ahead }} / {{ gitStatus.behind }}</b></article><article><small>工作区</small><b>{{ gitStatus.changedFiles.length }} 个文件</b></article></div><div class="git-remote-actions"><button class="button secondary" :disabled="loading || !gitStatus.remoteUrl" @click="runGitAction(() => fetchGitRepository(selected!.path))">更新远程</button><button class="button primary" :disabled="loading || !gitStatus.remoteUrl" @click="runGitAction(() => pullGitRepository(selected!.path), '仅在工作区干净且可快进时拉取远程代码，是否继续？')">拉取代码</button><small>“更新远程”只刷新远程状态，不修改本地文件；“拉取代码”使用仅快进模式，避免自动产生合并提交。</small></div></section>
        <section v-if="gitStatus" class="git-branch-panel"><h3>分支操作</h3><div><label>切换本地分支<select v-model="switchBranch"><option v-for="branch in gitStatus.branches" :key="branch">{{ branch }}</option></select></label><button class="button secondary" :disabled="loading || switchBranch === gitStatus.currentBranch" @click="runGitAction(() => switchGitRepositoryBranch(selected!.path, switchBranch), `确认从 ${gitStatus.currentBranch} 切换到 ${switchBranch} 吗？`)">切换</button></div><div><label>合并来源分支<select v-model="mergeBranch"><option value="" disabled>请选择分支</option><option v-for="branch in mergeOptions" :key="branch">{{ branch }}</option></select></label><button class="button secondary" :disabled="loading || !mergeBranch" @click="runGitAction(() => mergeGitRepositoryBranch(selected!.path, mergeBranch), `确认将 ${mergeBranch} 合并到 ${gitStatus.currentBranch} 吗？工作区必须保持干净。`)">合并</button></div></section>
        <section class="git-credential-panel"><header><div><h3>默认 Git 凭据</h3><p>项目自身没有可用登录凭据时使用；只保存在 Windows 凭据库。</p></div><span :class="{ configured: gitStatus?.credential.configured }">{{ gitStatus?.credential.configured ? `已配置 · ${gitStatus.credential.username}` : "未配置" }}</span></header><div><input v-model="credentialUsername" placeholder="用户名（默认 lzsk）"><input v-model="credentialSecret" type="password" autocomplete="new-password" placeholder="密码或访问令牌"><button class="button primary" :disabled="loading || !credentialSecret" @click="saveCredential">保存凭据</button><button v-if="gitStatus?.credential.configured" class="button danger-button" @click="clearCredential">删除</button></div><small>GitHub 已不支持账号密码拉取，请在这里填写个人访问令牌；密码或令牌不会写入项目配置、数据库或日志。</small></section>
        <section class="commit-plan"><header><div><h3>提交修改</h3><p>自动按功能分组并生成 type(scope): 描述；提交前可以修改信息。</p></div><button class="button secondary" :disabled="loading || !gitStatus?.hasUncommittedChanges" @click="planCommit">{{ details.commitPlan ? "重新分析" : "生成提交建议" }}</button></header><p v-if="!gitStatus?.hasUncommittedChanges" class="git-empty">当前工作区干净，没有可提交修改。</p><div v-if="details.commitPlan && gitStatus?.hasUncommittedChanges" class="commit-plan-summary"><span>风险 {{ details.commitPlan.riskLevel }}</span><p>{{ details.commitPlan.summary }}</p><article v-for="group in details.commitPlan.groups" :key="group.id"><div class="commit-group-title"><b>{{ group.title }}</b><i :class="{ committed: group.status === 'committed' }">{{ group.status === "committed" ? "已提交" : `${group.files.length} 个文件` }}</i></div><input v-model="commitMessages[group.id]" :disabled="group.status === 'committed'" aria-label="提交信息"><small>{{ group.riskNotes }}</small><details><summary>查看文件</summary><p v-for="file in group.files" :key="file">{{ file }}</p></details><button class="button primary small" :disabled="loading || group.status === 'committed'" @click="commitGroup(group.id, group.files)">确认提交本组</button></article></div></section>
        <section class="git-history"><header><div><h3>提交历史与回退</h3><p>回退会创建一条新的 revert 提交，不改写已有历史。</p></div></header><article v-for="item in details.commits" :key="item.hash"><code>{{ item.hash.slice(0, 7) }}</code><span><b>{{ item.subject }}</b><small>{{ item.committedAt.slice(0, 10) }}</small></span><button class="text-button danger-text" :disabled="loading" @click="runGitAction(() => revertGitRepositoryCommit(selected!.path, item.hash), `确认回退提交 ${item.hash.slice(0, 7)} 吗？\n\n${item.subject}\n\n系统会创建一条新的回退提交，不会强制重置历史。`)">回退</button></article><p v-if="!details.commits.length" class="git-empty">暂无可读取提交历史。</p></section>
        <section v-if="gitOutput" class="git-output"><h3>最近执行结果</h3><pre>{{ gitOutput }}</pre></section>
      </template>
    </aside></div>
  </div>
</template>

<style scoped>
.asset-metrics{display:grid;grid-template-columns:repeat(4,1fr);gap:12px;margin-bottom:12px}.asset-metrics article{padding:15px 17px;min-height:92px}.asset-metrics small,.asset-metrics span{color:var(--muted)}.asset-metrics b{display:block;font-size:27px;margin:7px 0}.asset-metrics span{font-size:9px}.asset-metrics .warning b{color:var(--warning)}.asset-metrics .failed b{color:var(--danger)}.asset-workspace{overflow:hidden}.asset-toolbar{height:58px;border-bottom:1px solid var(--line);display:flex;align-items:center;gap:9px;padding:0 14px}.asset-toolbar label{flex:1;max-width:410px;height:35px;border:1px solid var(--line);border-radius:7px;display:flex;align-items:center;gap:8px;padding:0 10px;color:var(--muted)}.asset-toolbar input{flex:1;border:0;outline:0;background:transparent}.asset-toolbar select{height:35px;border:1px solid var(--line);border-radius:7px;background:var(--surface-2);padding:0 10px}.asset-toolbar>span{margin-left:auto;color:var(--muted)}.asset-table-wrap{max-height:590px;overflow:auto}.asset-table{width:100%;border-collapse:collapse}.asset-table th{text-align:left;color:var(--muted);font-size:9px;background:var(--surface-2);position:sticky;top:0;z-index:2}.asset-table th,.asset-table td{padding:11px 12px;border-bottom:1px solid var(--line)}.asset-table th.pin-column{width:54px}.asset-table tbody tr{cursor:pointer}.asset-table tbody tr:hover,.asset-table tbody tr.pinned{background:var(--primary-soft)}.asset-table td b,.asset-table td small{display:block;max-width:240px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.asset-table td small{color:var(--muted);font-size:9px;margin-top:5px}.pin-button{min-width:34px;height:30px;border:1px solid var(--line);border-radius:7px;background:var(--surface-2);color:var(--muted);padding:0 9px}.pin-button.active{color:var(--warning);border-color:color-mix(in srgb,var(--warning) 45%,var(--line));background:color-mix(in srgb,var(--warning) 10%,var(--surface))}.health-pill,.dirty-pill{display:inline-flex;padding:5px 8px;border-radius:12px;font-style:normal;font-size:9px}.health-pill.healthy{color:var(--success);background:color-mix(in srgb,var(--success) 12%,transparent)}.health-pill.warning,.dirty-pill{color:var(--warning);background:color-mix(in srgb,var(--warning) 12%,transparent)}.health-pill.failed{color:var(--danger);background:color-mix(in srgb,var(--danger) 12%,transparent)}.health-pill.unknown{color:var(--muted);background:var(--surface-2)}.clean-text{color:var(--success);font-size:10px}.asset-drawer{width:780px;height:100%;margin-left:auto;border-radius:0;overflow:auto;padding-bottom:30px}.asset-drawer>header{padding:18px 20px;border-bottom:1px solid var(--line);display:flex;justify-content:space-between}.asset-drawer h2{margin:4px 0}.asset-drawer header p,.asset-drawer header small{margin:0;color:var(--muted)}.drawer-header-actions{display:flex;align-items:flex-start;gap:8px}.asset-status-strip{display:flex;gap:8px;align-items:center;padding:12px 20px;background:var(--surface-2);border-bottom:1px solid var(--line)}.asset-status-strip>span:not(.health-pill){padding:5px 8px;border:1px solid var(--line);border-radius:12px;font-size:9px;color:var(--muted)}.asset-tabs{height:48px;padding:6px 20px;border-bottom:1px solid var(--line);display:flex;gap:6px}.asset-tabs button{height:34px;border:0;border-radius:7px;background:transparent;color:var(--muted);padding:0 16px}.asset-tabs button.active{background:var(--primary-soft);color:var(--primary);font-weight:800}.asset-overview,.asset-commands,.asset-form{padding:18px 20px;border-bottom:1px solid var(--line)}.asset-drawer h3{margin:0 0 13px}.asset-overview dl{display:grid;grid-template-columns:1fr 1fr;gap:9px}.asset-overview dl div{padding:11px;background:var(--surface-2);border-radius:7px}.asset-overview dt{color:var(--muted);font-size:9px}.asset-overview dd{margin:6px 0 0;line-height:1.5}.asset-commands>div{display:grid;grid-template-columns:55px 1fr;align-items:center;margin-bottom:7px}.asset-commands code{padding:8px;background:var(--surface-2);border-radius:6px}.asset-commands small{color:var(--muted)}.asset-form label{display:flex;flex-direction:column;gap:6px;color:var(--muted);font-size:10px;margin-bottom:11px}.asset-form input,.asset-form textarea{border:1px solid var(--line);border-radius:7px;background:var(--surface-2);padding:9px;outline:0}.asset-form input:focus,.asset-form textarea:focus{border-color:var(--primary)}.asset-form footer{display:flex;justify-content:flex-end;gap:8px}.asset-related{display:grid;grid-template-columns:1fr 1fr;gap:12px;padding:18px 20px}.asset-related>div{min-width:0}.asset-related button,.asset-related article{width:100%;border:1px solid var(--line);border-radius:7px;background:var(--surface-2);color:inherit;padding:9px;margin-bottom:7px;display:flex;align-items:center;gap:9px;text-align:left}.asset-related button span,.asset-related article span{min-width:0;flex:1;display:flex;flex-direction:column;gap:4px}.asset-related b{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.asset-related small,.asset-related em,.asset-related p{color:var(--muted);font-size:9px}.asset-related em{font-style:normal}.asset-related article code{color:var(--primary)}
.asset-row-actions{display:flex;align-items:center;gap:10px;white-space:nowrap}.muted-action{color:var(--muted)}.hidden-project-backdrop{z-index:230;align-items:center;justify-content:center;padding:30px}.hidden-project-dialog{width:min(720px,calc(100vw - 80px));max-height:min(720px,calc(100vh - 80px));overflow:hidden}.hidden-project-dialog>header{height:72px;padding:0 18px;border-bottom:1px solid var(--line);display:flex;align-items:center;justify-content:space-between}.hidden-project-dialog h2{margin:0 0 5px}.hidden-project-dialog header p{margin:0;color:var(--muted)}.hidden-project-list{max-height:620px;overflow:auto;padding:8px 18px 18px}.hidden-project-list article{min-height:68px;border-bottom:1px solid var(--line);display:flex;align-items:center;gap:12px}.hidden-project-list article>div{min-width:0;flex:1;display:flex;flex-direction:column;gap:6px}.hidden-project-list small{color:var(--muted);white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.git-dashboard,.git-branch-panel,.git-credential-panel,.commit-plan,.git-history,.git-output{padding:18px 20px;border-bottom:1px solid var(--line)}.git-dashboard>header,.git-credential-panel>header,.commit-plan>header,.git-history>header{display:flex;align-items:flex-start;justify-content:space-between;gap:12px}.git-dashboard header p,.git-credential-panel header p,.commit-plan header p,.git-history header p{margin:4px 0 0;color:var(--muted);font-size:10px;overflow-wrap:anywhere}.git-metrics{display:grid;grid-template-columns:repeat(4,1fr);gap:8px;margin:13px 0}.git-metrics article{min-width:0;padding:11px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2)}.git-metrics small{display:block;color:var(--muted);font-size:9px}.git-metrics b{display:block;margin-top:7px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.git-remote-actions{display:flex;align-items:center;gap:8px}.git-remote-actions small{flex:1;color:var(--muted);line-height:1.5}.git-branch-panel>div{display:grid;grid-template-columns:1fr auto;gap:8px;margin-top:9px}.git-branch-panel label{display:grid;grid-template-columns:110px 1fr;align-items:center;gap:8px;color:var(--muted)}.git-branch-panel select,.git-credential-panel input,.commit-plan-summary input{height:36px;border:1px solid var(--line);border-radius:7px;background:var(--surface-2);padding:0 10px;outline:0}.git-credential-panel>header>span{padding:5px 8px;border-radius:6px;background:var(--surface-2);color:var(--muted);font-size:9px}.git-credential-panel>header>span.configured{color:var(--success);background:color-mix(in srgb,var(--success) 12%,transparent)}.git-credential-panel>div{display:grid;grid-template-columns:140px minmax(180px,1fr) auto auto;gap:8px;margin:13px 0 8px}.git-credential-panel>small{color:var(--muted);line-height:1.5}.commit-plan-summary>span{display:inline-block;color:var(--warning);margin:10px 0}.commit-plan-summary>p{color:var(--muted)}.commit-plan-summary article{padding:11px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);margin-top:8px}.commit-group-title{display:flex;justify-content:space-between;align-items:center}.commit-group-title i{font-style:normal;color:var(--muted);font-size:9px}.commit-group-title i.committed{color:var(--success)}.commit-plan-summary input{width:100%;margin:10px 0 7px}.commit-plan-summary article small{color:var(--muted)}.commit-plan-summary details{margin:8px 0;color:var(--muted)}.commit-plan-summary details p{margin:4px 0;font-family:monospace;font-size:9px;overflow-wrap:anywhere}.git-history article{display:grid;grid-template-columns:64px minmax(0,1fr) auto;align-items:center;gap:9px;padding:10px 0;border-bottom:1px solid var(--line)}.git-history article>code{color:var(--primary)}.git-history article>span{min-width:0;display:flex;flex-direction:column;gap:4px}.git-history b{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.git-history small,.git-empty{color:var(--muted);font-size:9px}.danger-text{color:var(--danger)}.git-output pre{max-height:180px;margin:0;overflow:auto;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);padding:11px;white-space:pre-wrap;overflow-wrap:anywhere;color:var(--muted);font:10px/1.6 monospace}
@media(max-width:1350px){.asset-drawer{width:700px}.git-metrics{grid-template-columns:repeat(2,1fr)}.git-credential-panel>div{grid-template-columns:1fr 1fr}.git-credential-panel>div .button{width:100%}}
</style>
