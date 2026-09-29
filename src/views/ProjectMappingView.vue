<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { isTauriRuntime, listProjectProfiles, previewProjectProfile, saveProjectProfile, type ProjectMappingPreview, type ProjectProfile, type ProjectProfileUpdate } from "../services/backend";
import { backgroundRequest } from "../services/backendRequest";

const profiles = ref<ProjectProfile[]>([]);
const selectedId = ref("");
const query = ref("");
const loading = ref(false);
const message = ref("");
const error = ref("");
const form = ref<ProjectProfileUpdate>({ id:"", displayName:"", repositoryPath:"", tapdWorkspaceId:"", aliases:[], category:"" });
const aliasesText = ref("");
const preview = ref<ProjectMappingPreview | null>(null);
const previewLoading = ref(false);
const previewError = ref("");
let previewTimer: ReturnType<typeof setTimeout> | undefined;
let previewSequence = 0;
const draft = computed<ProjectProfileUpdate>(() => ({ ...form.value, aliases: aliasesText.value.split(/[\n,，]/).map(item=>item.trim()).filter(Boolean) }));
const impactKinds = [{ key:"conversations", label:"Codex 对话" }, { key:"testRuns", label:"测试记录" }, { key:"workSessions", label:"工时区间" }] as const;

async function loadPreview(sequence: number, value: ProjectProfileUpdate) {
  try {
    const result = await backgroundRequest(() => previewProjectProfile(value));
    if (sequence === previewSequence) preview.value = result;
  } catch(cause) {
    if (sequence === previewSequence) previewError.value = `影响预览读取失败：${String(cause)}`;
  } finally {
    if (sequence === previewSequence) previewLoading.value = false;
  }
}

function retryPreview() {
  clearTimeout(previewTimer);
  previewError.value = "";
  previewLoading.value = true;
  void loadPreview(++previewSequence, draft.value);
}

watch(draft, value => {
  clearTimeout(previewTimer);
  const sequence = ++previewSequence;
  preview.value = null;
  previewError.value = "";
  previewLoading.value = Boolean(value.id && isTauriRuntime());
  if (previewLoading.value) previewTimer = setTimeout(() => void loadPreview(sequence, value), 350);
}, { deep:true });

const filtered = computed(() => {
  const keyword=query.value.trim().toLowerCase();
  return keyword ? profiles.value.filter(item => [item.displayName,item.repositoryPath,item.tapdWorkspaceId,...item.aliases].some(value=>value.toLowerCase().includes(keyword))) : profiles.value;
});

function selectProfile(item:ProjectProfile) {
  selectedId.value=item.id;
  form.value={ id:item.id,displayName:item.displayName,repositoryPath:item.repositoryPath,tapdWorkspaceId:item.tapdWorkspaceId,aliases:[...item.aliases],category:item.category };
  aliasesText.value=item.aliases.join("\n");
  message.value=""; error.value="";
}

async function load() {
  if (!isTauriRuntime()) return;
  loading.value=true; error.value="";
  try {
    profiles.value=await listProjectProfiles();
    const current=profiles.value.find(item=>item.id===selectedId.value) || profiles.value[0];
    if(current) selectProfile(current);
  } catch(cause) { error.value=String(cause); }
  finally { loading.value=false; }
}

async function save() {
  if(!form.value.displayName.trim()) { error.value="项目名称不能为空。"; return; }
  if (previewLoading.value || !preview.value) { error.value="请等待影响预览完成后保存。"; return; }
  if (preview.value.blockingConflicts.length) { error.value="请先调整新增的冲突名称或别名。"; return; }
  loading.value=true; error.value=""; message.value="";
  try {
    const saved=await saveProjectProfile(draft.value);
    const index=profiles.value.findIndex(item=>item.id===saved.id);
    if(index>=0) profiles.value.splice(index,1,saved); else profiles.value.push(saved);
    selectProfile(saved);
    message.value="项目映射已保存，后续工时、Token、报告和待处理事项会使用这个规范名称。";
  } catch(cause) { error.value=String(cause); }
  finally { loading.value=false; }
}

onMounted(load);
onBeforeUnmount(() => { clearTimeout(previewTimer); previewSequence++; });
</script>

<template>
  <div class="view project-mapping-view">
    <header class="page-header"><div><h1>项目身份映射</h1><p>统一本地目录、Codex 与 TAPD 项目名称</p></div><div><RouterLink class="button secondary link-button" to="/projects">返回项目资产</RouterLink><button class="button primary" :disabled="loading" @click="load">{{ loading ? "同步中…" : "↻ 同步项目" }}</button></div></header>
    <div v-if="message || error" class="mapping-message" :class="{error:Boolean(error)}">{{ error || message }}</div>
    <section class="mapping-layout panel">
      <aside class="mapping-list">
        <header><div><h2>规范项目</h2><p>{{ profiles.length }} 个本地项目已纳入映射</p></div></header>
        <label class="mapping-search">⌕<input v-model="query" placeholder="搜索项目、路径或别名"></label>
        <div>
          <button v-for="item in filtered" :key="item.id" :disabled="loading" :class="{active:item.id===selectedId}" @click="selectProfile(item)"><span><b>{{ item.displayName }}</b><small>{{ item.repositoryPath || "未关联本地目录" }}</small></span><em>{{ item.aliases.length }} 个别名</em></button>
          <p v-if="!filtered.length">没有符合条件的项目。</p>
        </div>
      </aside>
      <form v-if="selectedId" class="mapping-editor" @submit.prevent="save">
        <header><div><h2>编辑项目身份</h2><p>规范名称用于统计，别名用于识别</p></div><span>唯一项目</span></header>
        <div class="mapping-form-grid">
          <label>规范项目名称<input v-model="form.displayName" maxlength="80" placeholder="例如：安全生产管理"></label>
          <label>分类<input v-model="form.category" maxlength="40" placeholder="例如：TB 业务系统"></label>
          <label class="wide">本地项目目录<input v-model="form.repositoryPath" readonly></label>
          <label>TAPD 项目 ID<input v-model="form.tapdWorkspaceId" placeholder="例如：37583308"></label>
          <label class="wide">识别别名<textarea v-model="aliasesText" rows="8" placeholder="每行一个，例如：&#10;scaq-client&#10;client&#10;F:\TB-project\scaq-client"></textarea><small>每行一个可识别名称或路径</small></label>
        </div>
        <section class="mapping-impact" aria-label="项目映射影响预览" :aria-busy="previewLoading">
          <h3>保存后关联变化</h3>
          <p v-if="previewLoading" role="status">正在核对历史记录…</p>
          <p v-else-if="previewError" class="mapping-impact-error" role="alert">{{ previewError }} <button type="button" class="button secondary small" @click="retryPreview">重试预览</button></p>
          <template v-else-if="preview">
            <div class="mapping-impact-counts"><article v-for="kind in impactKinds" :key="kind.key"><span>{{ kind.label }}</span><b>{{ preview.before[kind.key] }} → {{ preview.after[kind.key] }}</b><small>新增关联 {{ preview.added[kind.key] }} · 移出 {{ preview.removed[kind.key] }}</small></article></div>
            <p v-if="preview.blockingConflicts.length" class="mapping-impact-error" role="alert">存在新增冲突，请调整：{{ preview.blockingConflicts.join("；") }}</p>
            <p v-else-if="preview.conflicts.length" class="mapping-impact-warning">已有重名提示：{{ preview.conflicts.join("；") }}</p>
            <p>全库尚未匹配到规范项目：对话 {{ preview.unmatched.conversations }} · 测试 {{ preview.unmatched.testRuns }} · 工时 {{ preview.unmatched.workSessions }}。原始记录仍会保留。</p>
          </template>
        </section>
        <footer><span>保存后不会重写 Codex、Git 或 TAPD 原始记录。</span><button class="button primary" :disabled="loading || previewLoading || !preview || Boolean(preview.blockingConflicts.length)">{{ loading ? "保存中…" : "保存映射" }}</button></footer>
      </form>
      <div v-else class="mapping-empty"><b>尚未发现项目资产</b><p>先在项目资产中扫描 Git 仓库，再回来同步项目映射。</p></div>
    </section>
  </div>
</template>

<style scoped>
.mapping-message{margin-bottom:12px;border:1px solid color-mix(in srgb,var(--success) 35%,var(--line));border-radius:8px;background:color-mix(in srgb,var(--success) 8%,var(--surface));color:var(--success);padding:11px 14px}.mapping-message.error{border-color:color-mix(in srgb,var(--danger) 40%,var(--line));background:color-mix(in srgb,var(--danger) 8%,var(--surface));color:var(--danger)}
.mapping-layout{display:grid;grid-template-columns:360px minmax(0,1fr);min-height:650px;overflow:hidden}.mapping-list{border-right:1px solid var(--line)}.mapping-list>header,.mapping-editor>header{height:76px;padding:0 18px;border-bottom:1px solid var(--line);display:flex;align-items:center;justify-content:space-between}.mapping-list h2,.mapping-editor h2{margin:0 0 5px}.mapping-list p,.mapping-editor p{margin:0;color:var(--muted)}.mapping-search{height:38px;margin:12px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);display:flex;align-items:center;gap:8px;padding:0 11px;color:var(--muted)}.mapping-search input{min-width:0;flex:1;border:0;outline:0;background:transparent;color:var(--text)}.mapping-list>div{max-height:548px;overflow:auto}.mapping-list>div>button{width:100%;min-height:70px;border:0;border-bottom:1px solid var(--line);background:transparent;color:inherit;padding:11px 14px;display:flex;align-items:center;gap:10px;text-align:left}.mapping-list>div>button:hover,.mapping-list>div>button.active{background:var(--primary-soft)}.mapping-list button span{min-width:0;flex:1;display:grid;gap:6px}.mapping-list button b,.mapping-list button small{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.mapping-list button small,.mapping-list button em{color:var(--muted);font-size:9px}.mapping-list button em{font-style:normal;white-space:nowrap}.mapping-list>div>p{padding:24px;color:var(--muted)}
.mapping-editor>header span{padding:6px 9px;border-radius:7px;background:var(--primary-soft);color:var(--primary);font-size:10px}.mapping-form-grid{display:grid;grid-template-columns:1fr 1fr;gap:14px;padding:20px}.mapping-form-grid label{display:flex;flex-direction:column;gap:7px;color:var(--muted)}.mapping-form-grid label.wide{grid-column:1/3}.mapping-form-grid input,.mapping-form-grid textarea{border:1px solid var(--line);border-radius:8px;background:var(--surface-2);color:var(--text);padding:10px;outline:0}.mapping-form-grid input{height:40px}.mapping-form-grid input:focus,.mapping-form-grid textarea:focus{border-color:var(--primary)}.mapping-form-grid input[readonly]{opacity:.75}.mapping-form-grid small{font-size:9px;line-height:1.5}.mapping-editor>footer{padding:0 20px 20px;display:flex;align-items:center;justify-content:space-between;gap:12px}.mapping-editor>footer span{color:var(--muted);font-size:10px}.mapping-empty{display:grid;place-content:center;text-align:center;color:var(--muted)}.mapping-empty b{color:var(--text);font-size:16px}.mapping-empty p{margin:8px 0}
@media(max-width:1000px){.mapping-layout{grid-template-columns:300px minmax(0,1fr)}}
.mapping-impact{margin:0 20px 20px;padding:14px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);min-width:0}.mapping-impact h3{margin:0 0 12px;font-size:13px}.mapping-impact p{margin:10px 0 0;line-height:1.6;font-size:11px;overflow-wrap:anywhere}.mapping-impact-counts{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:10px}.mapping-impact-counts article{display:grid;gap:7px;min-width:0}.mapping-impact-counts span,.mapping-impact-counts small{color:var(--muted);font-size:10px;overflow-wrap:anywhere}.mapping-impact-counts b{font-size:16px;overflow-wrap:anywhere}.mapping-impact .mapping-impact-error{color:var(--danger)}.mapping-impact .mapping-impact-warning{color:var(--warning)}
@media(max-width:760px){.mapping-layout{grid-template-columns:minmax(0,1fr)}.mapping-list{border-right:0;border-bottom:1px solid var(--line)}.mapping-list>div{max-height:200px}.mapping-form-grid{grid-template-columns:minmax(0,1fr)}.mapping-form-grid label.wide{grid-column:auto}.mapping-editor>footer{flex-wrap:wrap}}
</style>
