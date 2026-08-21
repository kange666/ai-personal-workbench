<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useRoute } from "vue-router";
import {
  continueTapdCodexJob,
  getTapdStatus,
  isTauriRuntime,
  listRepositoryAssets,
  listTapdCodexJobs,
  listTapdItems,
  readTapdProcessReport,
  reviewTapdCodexJob,
  runTapdCodexJobTests,
  saveTapdAutoFixSettings,
  startTapdCodexJob,
  syncTapdItems,
  type RepositoryAsset,
  type TapdCodexJob,
  type TapdStatus,
  type TapdWorkItem,
} from "../services/backend";
import { compactDetailTitle } from "../utils/detailTitle";

const route = useRoute();
const status = ref<TapdStatus>({
  configured: false,
  source: "未配置",
  authMode: "token",
  workspaceId: "37583308",
  workspaceName: "安全生产管理",
  owner: "刘子世康",
  itemCount: 0,
  warnings: [],
  autoFixEnabled: false,
  autoFixRepositoryPath: "",
});
const items = ref<TapdWorkItem[]>([]);
const jobs = ref<TapdCodexJob[]>([]);
const repositories = ref<RepositoryAsset[]>([]);
const selected = ref<TapdWorkItem | null>(null);
const repositoryPath = ref("");
const typeFilter = ref("all");
const statusFilter = ref("待处理");
const search = ref("");
const loading = ref(false);
const message = ref("");
const error = ref("");
const reviewNote = ref("");
const codexNote = ref("");
const autoRepositoryPath = ref("");
const processReport = ref("");
const reportOpen = ref(false);
const reportLoading = ref(false);
let jobTimer = 0;

const typeLabels: Record<string, string> = {
  bug: "缺陷",
  task: "任务",
  story: "需求",
};
const counts = computed(() => ({
  all: items.value.length,
  bug: items.value.filter((item) => item.itemType === "bug").length,
  task: items.value.filter((item) => item.itemType === "task").length,
  story: items.value.filter((item) => item.itemType === "story").length,
}));
const statuses = computed(() => [
  ...new Set(items.value.map((item) => item.statusLabel).filter(Boolean)),
]);
const statusOptions = computed(() => [
  "待处理",
  ...statuses.value.filter((value) => value !== "待处理"),
]);
const filtered = computed(() =>
  items.value.filter((item) => {
    if (typeFilter.value !== "all" && item.itemType !== typeFilter.value)
      return false;
    if (statusFilter.value !== "all" && item.statusLabel !== statusFilter.value)
      return false;
    const term = search.value.trim().toLowerCase();
    return (
      !term ||
      `${item.title} ${item.description} ${item.owner}`
        .toLowerCase()
        .includes(term)
    );
  }),
);
const selectedJob = computed(() =>
  selected.value
    ? jobs.value.find((job) => job.itemId === selected.value?.id)
    : undefined,
);
const jobsByItem = computed(() => {
  const result = new Map<string, TapdCodexJob>();
  for (const job of jobs.value) {
    if (!result.has(job.itemId)) result.set(job.itemId, job);
  }
  return result;
});
const selectedTitle = computed(() =>
  compactDetailTitle(selected.value?.title || "TAPD 工作项", "TAPD"),
);
const running = computed(() =>
  jobs.value.some((job) => ["queued", "running"].includes(job.status)),
);

function formatTime(value?: string) {
  if (!value) return "尚未同步";
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : new Intl.DateTimeFormat("zh-CN", {
        month: "2-digit",
        day: "2-digit",
        hour: "2-digit",
        minute: "2-digit",
      }).format(date);
}
function shortPath(value: string) {
  return value.replace(/^.*[\\/]/, "");
}
function statusTagClass(item: TapdWorkItem) {
  const value = `${item.status} ${item.statusLabel}`.toLowerCase();
  if (
    [
      "完成",
      "已解决",
      "已实现",
      "已发布",
      "已关闭",
      "done",
      "closed",
      "resolved",
      "released",
      "verified",
    ].some((status) => value.includes(status))
  )
    return "done";
  if (
    [
      "处理中",
      "进行中",
      "实现中",
      "规划中",
      "doing",
      "progress",
      "developing",
      "planning",
    ].some((status) => value.includes(status))
  )
    return "active";
  if (
    ["阻塞", "挂起", "拒绝", "blocked", "suspended", "rejected"].some(
      (status) => value.includes(status),
    )
  )
    return "blocked";
  if (
    [
      "待处理",
      "待确认",
      "待开始",
      "新建",
      "open",
      "new",
      "created",
      "unconfirmed",
      "reopened",
    ].some((status) => value.includes(status))
  )
    return "pending";
  return "neutral";
}
function openItem(item: TapdWorkItem) {
  if (selected.value?.id !== item.id) {
    codexNote.value = "";
    processReport.value = "";
    reportOpen.value = false;
  }
  selected.value = item;
  if (!repositoryPath.value) {
    repositoryPath.value =
      repositories.value.find((repo) => repo.name.toLowerCase() === "client")
        ?.path ||
      repositories.value.find((repo) => repo.name.toLowerCase() === "app")
        ?.path ||
      repositories.value[0]?.path ||
      "";
  }
}
async function loadJobs() {
  if (isTauriRuntime()) jobs.value = await listTapdCodexJobs();
}
function refreshAfterBackgroundSync() {
  void load();
}
async function load() {
  if (!isTauriRuntime()) return;
  loading.value = true;
  error.value = "";
  try {
    [status.value, items.value, repositories.value, jobs.value] =
      await Promise.all([
        getTapdStatus(),
        listTapdItems(),
        listRepositoryAssets(),
        listTapdCodexJobs(),
      ]);
    autoRepositoryPath.value =
      status.value.autoFixRepositoryPath ||
      repositories.value.find((repo) => repo.name.toLowerCase() === "client")
        ?.path ||
      repositories.value[0]?.path ||
      "";
    const requested = String(route.query.item || "");
    if (requested)
      selected.value =
        items.value.find((item) => item.id === requested) || null;
  } catch (cause) {
    error.value = String(cause);
  } finally {
    loading.value = false;
  }
}
async function sync() {
  loading.value = true;
  error.value = "";
  message.value = "";
  try {
    const result = await syncTapdItems();
    const warning = result.warnings.length
      ? ` 当前令牌权限不完整：${result.warnings.join("；")}。`
      : "";
    const notificationText = result.notificationsCreated
      ? ` 新增 ${result.notificationsCreated} 条未读消息。`
      : "";
    const autoText = result.autoJobsStarted
      ? ` 已自动排队处理 ${result.autoJobsStarted} 个新缺陷。`
      : result.autoJobsSkipped
        ? ` ${result.autoJobsSkipped} 个新缺陷未进入自动处理，请查看提示。`
        : "";
    message.value = `同步完成：${result.bugs} 个缺陷、${result.tasks} 个任务、${result.stories} 个需求。${notificationText}${autoText}${warning}`;
    await load();
    window.dispatchEvent(new CustomEvent("tapd-items-synced"));
  } catch (cause) {
    error.value = String(cause);
  } finally {
    loading.value = false;
  }
}
async function updateAutoFix(enabled: boolean) {
  loading.value = true;
  error.value = "";
  message.value = "";
  try {
    const saved = await saveTapdAutoFixSettings(
      enabled,
      autoRepositoryPath.value,
    );
    status.value.autoFixEnabled = saved.enabled;
    status.value.autoFixRepositoryPath = saved.repositoryPath;
    autoRepositoryPath.value = saved.repositoryPath;
    message.value = saved.enabled
      ? "新缺陷自动修改已开启；后续同步发现的新缺陷会按验证、修复、复测流程串行处理。"
      : "新缺陷自动修改已关闭，现有任务不会被中断。";
  } catch (cause) {
    error.value = String(cause);
  } finally {
    loading.value = false;
  }
}
async function persistAutoRepository() {
  await updateAutoFix(status.value.autoFixEnabled);
}
async function sendToCodex() {
  if (!selected.value || !repositoryPath.value) return;
  loading.value = true;
  error.value = "";
  message.value = "";
  try {
    const job = await startTapdCodexJob(
      selected.value.id,
      repositoryPath.value,
      codexNote.value,
    );
    jobs.value.unshift(job);
    codexNote.value = "";
    message.value = "已发送给 Codex，完成后会在工作台产生未读提醒。";
  } catch (cause) {
    error.value = String(cause);
  } finally {
    loading.value = false;
  }
}
async function runProjectTests() {
  if (!selectedJob.value) return;
  loading.value = true;
  error.value = "";
  message.value = "";
  try {
    const updated = await runTapdCodexJobTests(selectedJob.value.id);
    jobs.value = jobs.value.map((item) =>
      item.id === updated.id ? updated : item,
    );
    message.value = updated.testSummary.startsWith("项目测试通过")
      ? "项目测试通过，可以继续确认结果。"
      : "项目测试未通过，请查看测试输出后决定是否要求继续修改。";
  } catch (cause) {
    error.value = String(cause);
  } finally {
    loading.value = false;
  }
}
async function showProcessReport() {
  if (!selectedJob.value) return;
  reportOpen.value = true;
  reportLoading.value = true;
  error.value = "";
  try {
    processReport.value = await readTapdProcessReport(selectedJob.value.id);
  } catch (cause) {
    processReport.value = "";
    error.value = String(cause);
  } finally {
    reportLoading.value = false;
  }
}
async function copyProcessReport() {
  if (!processReport.value) return;
  await navigator.clipboard.writeText(processReport.value);
  message.value = "处理报告已复制。";
}
async function reviewResult(decision: "accepted" | "changes_requested") {
  if (!selectedJob.value) return;
  if (decision === "changes_requested" && !reviewNote.value.trim()) {
    error.value = "请先填写需要继续修改的说明。";
    return;
  }
  if (
    decision === "accepted" &&
    !window.confirm(
      "确认后会把该 TAPD 缺陷更新为“已解决”并完成本地归档，是否继续？",
    )
  )
    return;
  loading.value = true;
  error.value = "";
  message.value = "";
  try {
    if (decision === "changes_requested") {
      const updated = await continueTapdCodexJob(
        selectedJob.value.id,
        reviewNote.value,
      );
      jobs.value = jobs.value.map((item) =>
        item.id === updated.id ? updated : item,
      );
      message.value =
        "补充说明已再次发送给原 Codex 任务，完成后会生成新的未读提醒。";
    } else {
      const updated = await reviewTapdCodexJob(
        selectedJob.value.id,
        decision,
        reviewNote.value,
      );
      jobs.value = jobs.value.map((item) =>
        item.id === updated.id ? updated : item,
      );
      items.value = items.value.map((item) =>
        item.id === updated.itemId
          ? { ...item, status: "resolved", statusLabel: "已解决" }
          : item,
      );
      selected.value = null;
      message.value =
        "TAPD 已更新为“已解决”，结果已归档，Codex、Git、今日日报和知识库关联数据已刷新。";
    }
    reviewNote.value = "";
  } catch (cause) {
    error.value = String(cause);
  } finally {
    loading.value = false;
  }
}
onMounted(async () => {
  await load();
  if (status.value.configured && !items.value.length) await sync();
  jobTimer = window.setInterval(() => {
    if (running.value) void loadJobs();
  }, 3000);
  window.addEventListener("tapd-background-synced", refreshAfterBackgroundSync);
});
onBeforeUnmount(() => {
  window.clearInterval(jobTimer);
  window.removeEventListener("tapd-background-synced", refreshAfterBackgroundSync);
});
</script>

<template>
  <div class="view tapd-view">
    <header class="page-header">
      <div>
        <h1>TAPD 工作</h1>
        <p>安全生产管理 · 同步“我的工作”，选择工作项后可发送给 Codex</p>
      </div>
      <div>
        <a
          class="button secondary link-button"
          href="https://www.tapd.cn/37583308"
          target="_blank"
          rel="noreferrer"
          >打开 TAPD</a
        ><button
          class="button primary"
          :disabled="loading || !status.configured"
          @click="sync"
        >
          {{ loading ? "同步中…" : "↻ 同步项目" }}
        </button>
      </div>
    </header>
    <div
      v-if="message || error"
      class="scan-message"
      :class="{ error: Boolean(error) }"
    >
      {{ error || message }}
    </div>
    <section class="panel tapd-auto-fix">
      <div>
        <b>新缺陷自动修改</b>
        <p>
          自动按“修改前静态与动态验证 → 最小方案 → 实施 →
          修改后静态与动态复测”处理，只处理后续同步发现的新缺陷。
        </p>
      </div>
      <div class="tapd-auto-controls">
        <select
          v-model="autoRepositoryPath"
          :disabled="loading || !repositories.length"
          aria-label="自动处理使用的本地项目"
          @change="persistAutoRepository"
        >
          <option value="" disabled>选择自动处理项目</option>
          <option v-for="repo in repositories" :key="repo.path" :value="repo.path">
            {{ repo.name }} · {{ repo.path }}
          </option>
        </select>
        <button
          class="button"
          :class="status.autoFixEnabled ? 'primary' : 'secondary'"
          :disabled="loading || !status.configured"
          @click="updateAutoFix(!status.autoFixEnabled)"
        >
          自动修改：{{ status.autoFixEnabled ? "开" : "关" }}
        </button>
      </div>
    </section>
    <section v-if="!status.configured" class="panel tapd-config-notice">
      <div>
        <b>还差一步：配置 TAPD OpenAPI</b>
        <p>
          浏览器登录只能用于查看页面，工作台持续同步需要 TAPD 提供的 API
          用户名和密码。凭据只保存到 Windows 凭据库。
        </p>
      </div>
      <RouterLink class="button primary link-button" to="/settings"
        >前往设置</RouterLink
      >
    </section>
    <section class="tapd-summary">
      <article class="panel">
        <small>项目</small><b>{{ status.workspaceName }}</b
        ><span>{{ status.workspaceId }}</span>
      </article>
      <article class="panel">
        <small>我的工作</small><b>{{ counts.all }}</b
        ><span>负责人：{{ status.owner }}</span>
      </article>
      <article class="panel">
        <small>缺陷 / 任务 / 需求</small
        ><b>{{ counts.bug }} / {{ counts.task }} / {{ counts.story }}</b
        ><span>本地只读缓存</span>
      </article>
      <article class="panel">
        <small>最近同步</small><b>{{ formatTime(status.lastSyncedAt) }}</b
        ><span>{{ status.configured ? status.source : "未配置连接" }}</span>
      </article>
    </section>
    <section class="panel tapd-work-list">
      <header>
        <div class="tapd-tabs">
          <button
            v-for="tab in [
              { id: 'all', label: '全部' },
              { id: 'bug', label: '缺陷' },
              { id: 'task', label: '任务' },
              { id: 'story', label: '需求' },
            ]"
            :key="tab.id"
            :class="{ active: typeFilter === tab.id }"
            @click="typeFilter = tab.id"
          >
            {{ tab.label }} <i>{{ counts[tab.id as keyof typeof counts] }}</i>
          </button>
        </div>
        <div class="tapd-filters">
          <select v-model="statusFilter">
            <option value="all">全部状态</option>
            <option v-for="value in statusOptions" :key="value" :value="value">
              {{ value }}
            </option></select
          ><label
            >⌕<input v-model="search" placeholder="搜索标题、描述或处理人"
          /></label>
        </div>
      </header>
      <div class="tapd-table-head">
        <span>类型</span><span>标题</span><span>状态</span><span>优先级</span
        ><span>处理人</span><span>预计结束</span><span>更新时间</span>
      </div>
      <button
        v-for="item in filtered"
        :key="item.id"
        class="tapd-row"
        @click="openItem(item)"
      >
        <span
          ><i :class="item.itemType">{{ typeLabels[item.itemType] }}</i></span
        ><span
          ><b>{{ item.title }}</b
          ><small
            >#{{ item.id
            }}{{ item.description ? ` · ${item.description}` : "" }}<i
              v-if="jobsByItem.get(item.id)?.processReportPath"
              class="tapd-report-mark"
              >处理报告</i
            ></small
          ></span
        ><span
          ><em :class="statusTagClass(item)">{{ item.statusLabel }}</em></span
        ><span>{{ item.priority || "-" }}</span
        ><span>{{ item.owner || "-" }}</span
        ><span>{{ item.dueDate || "-" }}</span
        ><span>{{ item.modifiedAt || item.createdAt || "-" }}</span>
      </button>
      <p v-if="!filtered.length && !loading" class="panel-empty">
        {{
          status.configured
            ? "当前筛选条件下没有工作项。"
            : "配置 OpenAPI 后即可同步安全生产管理项目。"
        }}
      </p>
    </section>
    <div
      v-if="selected"
      class="activity-backdrop"
      @click.self="selected = null"
    >
      <aside class="activity-drawer panel tapd-drawer">
        <header>
          <div>
            <small
              >{{ typeLabels[selected.itemType] }} · #{{ selected.id }}</small
            >
            <h2 :title="selected.title">{{ selectedTitle }}</h2>
            <p>
              {{ selected.statusLabel }} ·
              {{ selected.owner || "未指定处理人" }}
            </p>
          </div>
          <button class="icon-button" @click="selected = null">×</button>
        </header>
        <div class="tapd-detail-meta">
          <span
            ><small>优先级</small
            ><b>{{ selected.priority || "未设置" }}</b></span
          ><span
            ><small>预计开始</small
            ><b>{{ selected.beginDate || "未设置" }}</b></span
          ><span
            ><small>预计结束</small
            ><b>{{ selected.dueDate || "未设置" }}</b></span
          ><span
            ><small>创建人</small
            ><b>{{ selected.creator || "未记录" }}</b></span
          >
        </div>
        <section>
          <h3>详细描述</h3>
          <p>
            {{
              selected.description ||
              "TAPD 未填写详细描述，请结合标题和项目代码确认。"
            }}
          </p>
          <a :href="selected.sourceUrl" target="_blank" rel="noreferrer"
            >在 TAPD 查看原始工作项 →</a
          >
        </section>
        <section class="tapd-codex-panel">
          <h3>发送给 Codex</h3>
          <p>Codex 会同时参考 TAPD 内容和你的补充备注，不会自动提交或推送。</p>
          <select v-model="repositoryPath">
            <option value="" disabled>选择本地项目</option>
            <option
              v-for="repo in repositories"
              :key="repo.path"
              :value="repo.path"
            >
              {{ repo.name }} · {{ repo.path }}
            </option></select
          ><label class="tapd-codex-note"
            ><span>补充备注（选填）</span
            ><textarea
              v-model="codexNote"
              rows="4"
              maxlength="4000"
              placeholder="补充业务背景、具体修改要求、参考页面或验收标准…"
            ></textarea
            ><small>{{ codexNote.length }} / 4000</small></label
          ><button
            class="button primary"
            :disabled="
              loading ||
              !repositoryPath ||
              ['queued', 'running'].includes(selectedJob?.status || '')
            "
            @click="sendToCodex"
          >
            {{
              ["queued", "running"].includes(selectedJob?.status || "")
                ? "Codex 执行中…"
                : "发送给 Codex"
            }}</button
          ><small v-if="repositoryPath"
            >目标：{{ shortPath(repositoryPath) }}</small
          >
        </section>
        <section v-if="selectedJob" class="tapd-job-result">
          <h3>
            Codex 结果
            <i :class="selectedJob.status">{{
              selectedJob.status === "completed"
                ? "已完成"
                : selectedJob.status === "failed"
                  ? "失败"
                  : selectedJob.status === "queued"
                    ? "排队中"
                    : "执行中"
            }}</i>
          </h3>
          <div class="tapd-report-actions">
            <span
              >{{ selectedJob.triggerSource === "auto" ? "自动处理" : "人工发送" }} ·
              过程报告将保存在本地 Markdown 文件</span
            ><button
              class="button secondary small"
              :disabled="reportLoading || !selectedJob.processReportPath"
              @click="showProcessReport"
            >
              {{ reportLoading ? "读取中…" : "查看处理报告" }}
            </button>
          </div>
          <div v-if="reportOpen" class="tapd-process-report">
            <header>
              <b>处理过程与结果</b
              ><div>
                <button
                  class="button secondary small"
                  :disabled="!processReport"
                  @click="copyProcessReport"
                >
                  复制 Markdown</button
                ><button class="icon-button" @click="reportOpen = false">×</button>
              </div>
            </header>
            <pre>{{ processReport || "报告正在生成，任务完成后重新打开即可查看。" }}</pre>
          </div>
          <pre v-if="selectedJob.output || selectedJob.errorMessage">{{
            selectedJob.output || selectedJob.errorMessage
          }}</pre>
          <p v-else>任务正在后台执行，完成后将生成未读提醒。</p>
          <div v-if="selectedJob.status === 'completed'" class="tapd-closure">
            <div>
              <b>Git 变更证据</b
              ><small
                >{{ selectedJob.changedFiles.length }} 个文件{{
                  selectedJob.baselineWorktree
                    ? " · 启动前已有未提交改动，确认时请注意区分"
                    : ""
                }}</small
              ><code
                v-for="file in selectedJob.changedFiles.slice(0, 12)"
                :key="file"
                >{{ file }}</code
              ><small v-if="!selectedJob.changedFiles.length"
                >未检测到工作区或提交变化。</small
              >
            </div>
            <div>
              <b>项目测试</b>
              <pre v-if="selectedJob.testSummary">{{
                selectedJob.testSummary
              }}</pre>
              <button
                v-else
                class="button secondary"
                :disabled="loading"
                @click="runProjectTests"
              >
                运行项目资产中的测试命令
              </button>
            </div>
            <div>
              <b>人工确认</b
              ><small
                >“需要继续修改”会把下方说明发回原 Codex
                任务；“确认完成并归档”会先将 TAPD 缺陷更新为“已解决”。不会自动提交或推送代码。</small
              ><textarea
                v-model="reviewNote"
                rows="2"
                maxlength="4000"
                placeholder="继续修改时必填：说明还需要修改的内容；确认完成时可填写验收备注"
              ></textarea>
              <div class="settings-actions">
                <button
                  class="button secondary"
                  :disabled="loading || !reviewNote.trim()"
                  @click="reviewResult('changes_requested')"
                >
                  需要继续修改</button
                ><button
                  class="button primary"
                  :disabled="loading"
                  @click="reviewResult('accepted')"
                >
                  确认完成并归档
                </button>
              </div>
              <p v-if="selectedJob.reviewStatus !== 'pending'">
                当前结论：{{
                  selectedJob.reviewStatus === "accepted"
                    ? "已确认完成"
                    : "需要继续修改"
                }}{{
                  selectedJob.reviewNote ? ` · ${selectedJob.reviewNote}` : ""
                }}
              </p>
            </div>
          </div>
        </section>
      </aside>
    </div>
  </div>
</template>
