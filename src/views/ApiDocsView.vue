<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  getApiEndpoint,
  getApifoxCredentialStatus,
  isTauriRuntime,
  listApiEndpoints,
  listApiSources,
  listProjectProfiles,
  removeApiSource,
  renderApiEndpointMarkdown,
  saveApiEndpointToKnowledge,
  saveApiSource,
  syncAllApiSources,
  syncApiSource,
  type ApiEndpointDetail,
  type ApiEndpointSummary,
  type ApiSource,
  type ApifoxCredentialStatus,
  type ProjectProfile,
} from "../services/backend";

type JsonObject = Record<string, unknown>;
interface SchemaRow { name:string; type:string; required:boolean; description:string; example:string }

const route = useRoute();
const router = useRouter();
const credential = ref<ApifoxCredentialStatus>({ configured:false, source:"未配置" });
const profiles = ref<ProjectProfile[]>([]);
const sources = ref<ApiSource[]>([]);
const selectedSourceId = ref("");
const endpoints = ref<ApiEndpointSummary[]>([]);
const selected = ref<ApiEndpointDetail | null>(null);
const loading = ref(false);
const syncingSourceId = ref("");
const syncingAll = ref(false);
const message = ref("");
const error = ref("");
const query = ref("");
const method = ref("");
const tag = ref("");
const deprecated = ref<"all"|"active"|"deprecated">("all");
const configOpen = ref(false);
const config = reactive({ id:"", projectProfileId:"", externalProjectId:"" });

function asObject(value:unknown):JsonObject { return value && typeof value === "object" && !Array.isArray(value) ? value as JsonObject : {}; }
function asArray(value:unknown):unknown[] { return Array.isArray(value) ? value : []; }
function asText(value:unknown):string { return typeof value === "string" ? value : ""; }
function asBoolean(value:unknown):boolean { return value === true; }
function pretty(value:unknown):string { return value === undefined ? "" : typeof value === "string" ? value : JSON.stringify(value,null,2); }
function normalizePath(value:string) { return value.trim().replaceAll("\\","/").replace(/\/$/,"").toLowerCase(); }

const activeSource = computed(() => sources.value.find(item=>item.id===selectedSourceId.value) || null);
const usedProfileIds = computed(() => new Set(sources.value.filter(item=>item.id!==config.id).map(item=>item.projectProfileId)));
const configurableProfiles = computed(() => profiles.value.filter(item=>!usedProfileIds.value.has(item.id)));
const methods = computed(() => [...new Set(endpoints.value.map(item=>item.method))].sort());
const tags = computed(() => [...new Set(endpoints.value.flatMap(item=>item.tags))].sort((a,b)=>a.localeCompare(b,"zh-CN")));
const filtered = computed(() => {
  const needle=query.value.trim().toLowerCase();
  return endpoints.value.filter(item => {
    if (method.value && item.method!==method.value) return false;
    if (tag.value && !item.tags.includes(tag.value)) return false;
    if (deprecated.value==="active" && item.deprecated) return false;
    if (deprecated.value==="deprecated" && !item.deprecated) return false;
    return !needle || `${item.title} ${item.path} ${item.description} ${item.operationId} ${item.tags.join(" ")}`.toLowerCase().includes(needle);
  });
});
const parameters = computed(() => asArray(asObject(selected.value?.document).parameters).map(value=>asObject(value)));
const requestBody = computed(() => asObject(asObject(selected.value?.document).requestBody));
const requestContent = computed(() => Object.entries(asObject(requestBody.value.content)).map(([contentType,value])=>({contentType,body:asObject(value)})));
const responses = computed(() => Object.entries(asObject(asObject(selected.value?.document).responses)).map(([status,value])=>({status,response:asObject(value)})));
const securityNames = computed(() => [...new Set(asArray(asObject(selected.value?.document).security).flatMap(value=>Object.keys(asObject(value))))]);
const warnings = computed(() => asArray(asObject(selected.value?.document).warnings).map(asText).filter(Boolean));

function schemaType(schema:JsonObject):string {
  const kind=asText(schema.type);
  if(kind==="array") return `array<${schemaType(asObject(schema.items))}>`;
  const format=asText(schema.format);
  if(kind) return format ? `${kind}(${format})` : kind;
  if(schema.oneOf) return "oneOf";
  if(schema.anyOf) return "anyOf";
  if(schema.allOf) return "allOf";
  return Object.keys(asObject(schema.properties)).length ? "object" : "未知";
}
function schemaRows(schemaValue:unknown,prefix="",required=false,depth=0):SchemaRow[] {
  if(depth>6) return [];
  const schema=asObject(schemaValue);
  const rows:SchemaRow[]=[];
  if(prefix) rows.push({name:prefix,type:schemaType(schema),required,description:asText(schema.description),example:pretty(schema.example)});
  const requiredNames=new Set(asArray(schema.required).map(asText));
  for(const [name,child] of Object.entries(asObject(schema.properties))) {
    const path=prefix ? `${prefix}.${name}` : name;
    rows.push(...schemaRows(child,path,requiredNames.has(name),depth+1));
  }
  if(asText(schema.type)==="array" && schema.items) rows.push(...schemaRows(schema.items,`${prefix}[]`,required,depth+1));
  return rows;
}
function parameterExample(item:JsonObject) { return pretty(item.example ?? asObject(item.schema).example); }
function responseContents(response:JsonObject) { return Object.entries(asObject(response.content)).map(([contentType,value])=>({contentType,body:asObject(value)})); }
function formatTime(value?:string) { return value ? new Intl.DateTimeFormat("zh-CN",{month:"numeric",day:"numeric",hour:"2-digit",minute:"2-digit"}).format(new Date(value)) : "从未同步"; }
function statusText(value:ApiSource["syncStatus"]) { return ({never:"从未同步",syncing:"正在同步",ready:"同步成功",stale:"使用旧缓存",error:"同步失败"} as Record<string,string>)[value] || value; }

async function loadEndpoints(sourceId:string, preferredEndpointId="") {
  endpoints.value = sourceId && isTauriRuntime() ? await listApiEndpoints(sourceId) : [];
  const target=preferredEndpointId || (selected.value?.sourceId===sourceId ? selected.value.id : "") || endpoints.value[0]?.id || "";
  selected.value = target && endpoints.value.some(item=>item.id===target) ? await getApiEndpoint(target) : null;
  if(selected.value) await router.replace({query:{...route.query,project:selected.value.projectProfileId,endpoint:selected.value.id}});
}

async function load() {
  if(!isTauriRuntime()) return;
  loading.value=true; error.value="";
  try {
    [credential.value,profiles.value,sources.value]=await Promise.all([getApifoxCredentialStatus(),listProjectProfiles(),listApiSources()]);
    const requestedEndpoint=String(route.query.endpoint || "");
    let requestedDetail:ApiEndpointDetail|null=null;
    if(requestedEndpoint) { try { requestedDetail=await getApiEndpoint(requestedEndpoint); } catch { /* 已删除的旧链接回退到项目列表。 */ } }
    const projectId=String(route.query.project || requestedDetail?.projectProfileId || "");
    const projectPath=normalizePath(String(route.query.projectPath || ""));
    const target=sources.value.find(item=>item.id===requestedDetail?.sourceId)
      || sources.value.find(item=>item.projectProfileId===projectId)
      || sources.value.find(item=>projectPath && normalizePath(item.repositoryPath)===projectPath)
      || sources.value.find(item=>item.id===selectedSourceId.value)
      || sources.value[0];
    selectedSourceId.value=target?.id || "";
    if(target) await loadEndpoints(target.id,requestedDetail?.id || requestedEndpoint);
  } catch(cause) { error.value=String(cause); }
  finally { loading.value=false; }
}

async function selectSource(item:ApiSource) {
  selectedSourceId.value=item.id; query.value="";method.value="";tag.value="";deprecated.value="all";error.value="";
  try { await loadEndpoints(item.id); }
  catch(cause){error.value=String(cause);}
}
async function selectEndpoint(item:ApiEndpointSummary) {
  error.value="";
  try { selected.value=await getApiEndpoint(item.id); await router.replace({query:{project:selected.value.projectProfileId,endpoint:item.id}}); }
  catch(cause){error.value=String(cause);}
}
function openCreate(source?:ApiSource) {
  config.id=source?.id || ""; config.projectProfileId=source?.projectProfileId || configurableProfiles.value[0]?.id || ""; config.externalProjectId=source?.externalProjectId || ""; configOpen.value=true;error.value="";message.value="";
}
async function persistSource() {
  loading.value=true;error.value="";message.value="";
  try { const saved=await saveApiSource({...config});configOpen.value=false;message.value="项目关联已保存，可以开始同步接口文档。";await load();await selectSource(sources.value.find(item=>item.id===saved.id) || saved); }
  catch(cause){error.value=String(cause);}finally{loading.value=false;}
}
async function removeSource(item:ApiSource) {
  if(!confirm(`确定移除“${item.projectName}”的 Apifox 关联和本地接口缓存吗？Apifox 原项目及知识库快照不会删除。`)) return;
  loading.value=true;error.value="";
  try { await removeApiSource(item.id);message.value="项目关联和本地接口缓存已移除。";selected.value=null;await load(); }
  catch(cause){error.value=String(cause);}finally{loading.value=false;}
}
async function syncOne(item:ApiSource) {
  syncingSourceId.value=item.id;error.value="";message.value="";
  try { const result=await syncApiSource(item.id);message.value=`${item.projectName} 同步完成：新增 ${result.added}、更新 ${result.updated}、移除 ${result.removed}，共 ${result.total} 个接口。`;await load(); }
  catch(cause){error.value=String(cause);await load();}
  finally{syncingSourceId.value="";}
}
async function syncAll() {
  syncingAll.value=true;error.value="";message.value="";
  try { const results=await syncAllApiSources();const failed=results.filter(item=>item.status!=="ready");message.value=`已同步 ${results.length-failed.length}/${results.length} 个项目${failed.length ? `，${failed.length} 个项目保留旧缓存或同步失败` : ""}。`;await load(); }
  catch(cause){error.value=String(cause);}finally{syncingAll.value=false;}
}
async function copyMarkdown() {
  if(!selected.value) return;
  try { const value=await renderApiEndpointMarkdown(selected.value.id);await navigator.clipboard.writeText(value);message.value="完整 Markdown 接口文档已复制，敏感示例值已遮盖。"; }
  catch(cause){error.value=String(cause);}
}
async function saveKnowledge() {
  if(!selected.value) return;
  try { const item=await saveApiEndpointToKnowledge(selected.value.id);message.value="接口文档快照已保存到知识库。";await router.push(`/knowledge?item=${encodeURIComponent(item.id)}`); }
  catch(cause){error.value=String(cause);}
}
function openTesting() { if(selected.value?.repositoryPath) void router.push({path:"/testing",query:{project:selected.value.repositoryPath}}); }

onMounted(load);
</script>

<template>
  <div class="view api-docs-view">
    <header class="page-header"><div><h1>接口文档中心</h1><p>按规范项目同步 Apifox OpenAPI 文档，统一查询、复制和复用</p></div><div><RouterLink v-if="!credential.configured" class="button secondary link-button" to="/settings">配置 Apifox 令牌</RouterLink><button class="button secondary" :disabled="syncingAll || !sources.length || !credential.configured" @click="syncAll">{{ syncingAll ? "同步中…" : "同步全部" }}</button><button class="button primary" @click="openCreate()">＋ 关联项目</button></div></header>
    <div v-if="message || error" class="scan-message" :class="{error:Boolean(error)}">{{ error || message }}</div>
    <section v-if="!credential.configured" class="panel api-onboarding"><b>先配置 Apifox API 访问令牌</b><p>令牌只保存在 Windows 凭据库，接口文档缓存才会写入本地 SQLite。</p><RouterLink class="button primary link-button" to="/settings">前往设置</RouterLink></section>
    <section class="panel api-docs-layout">
      <aside class="api-source-panel">
        <header><div><b>规范项目</b><small>{{ sources.length }} 个 Apifox 关联</small></div><button class="icon-button" title="新增关联" @click="openCreate()">＋</button></header>
        <div class="api-source-list">
          <button v-for="item in sources" :key="item.id" :class="{active:item.id===selectedSourceId}" @click="selectSource(item)"><span><b>{{ item.projectName }}</b><small>{{ item.documentTitle || `Apifox ${item.externalProjectId}` }}</small><em :class="item.syncStatus">{{ statusText(item.syncStatus) }} · {{ item.endpointCount }} 个</em></span><i>›</i></button>
          <p v-if="!sources.length">尚未关联项目。点击“关联项目”后填写 Apifox 项目 ID。</p>
        </div>
        <footer v-if="activeSource"><button class="text-button" @click="openCreate(activeSource)">编辑关联</button><button class="text-button danger-text" @click="removeSource(activeSource)">移除</button></footer>
      </aside>

      <main class="api-endpoint-panel">
        <header><div><b>{{ activeSource?.projectName || "接口列表" }}</b><small>{{ activeSource ? `${formatTime(activeSource.lastSyncedAt)} · OpenAPI ${activeSource.openapiVersion || '待读取'}` : '请选择项目' }}</small></div><button v-if="activeSource" class="button secondary small" :disabled="syncingSourceId===activeSource.id || !credential.configured" @click="syncOne(activeSource)">{{ syncingSourceId===activeSource.id ? "同步中…" : "↻ 同步" }}</button></header>
        <div v-if="activeSource?.lastError" class="api-cache-warning"><b>{{ activeSource.syncStatus==='stale' ? '正在使用上次成功缓存' : '同步未完成' }}</b><span>{{ activeSource.lastError }}</span></div>
        <div class="api-filters"><label>⌕<input v-model="query" placeholder="搜索名称、路径、描述或 Operation ID"></label><select v-model="method"><option value="">全部方法</option><option v-for="item in methods" :key="item">{{ item }}</option></select><select v-model="tag"><option value="">全部标签</option><option v-for="item in tags" :key="item">{{ item }}</option></select><select v-model="deprecated"><option value="all">全部状态</option><option value="active">正常接口</option><option value="deprecated">已弃用</option></select></div>
        <div class="api-endpoint-list"><button v-for="item in filtered" :key="item.id" :class="{active:item.id===selected?.id}" @click="selectEndpoint(item)"><span class="api-method" :class="item.method.toLowerCase()">{{ item.method }}</span><span><b>{{ item.title }}</b><code>{{ item.path }}</code><small>{{ item.tags.join(' · ') || item.operationId || '未分组' }}</small></span><em v-if="item.deprecated">弃用</em></button><p v-if="activeSource && !filtered.length">{{ endpoints.length ? "没有符合筛选条件的接口。" : "尚无缓存，请点击同步。" }}</p><p v-if="!activeSource">先从左侧选择或关联一个项目。</p></div>
      </main>

      <article class="api-detail-panel">
        <template v-if="selected">
          <header><div><span class="api-method large" :class="selected.method.toLowerCase()">{{ selected.method }}</span><div><h2>{{ selected.title }}</h2><code>{{ selected.path }}</code></div></div><div><button class="button secondary small" :disabled="!selected.repositoryPath" @click="openTesting">项目测试</button><button class="button secondary small" @click="saveKnowledge">保存到知识库</button><button class="button primary small" @click="copyMarkdown">复制 Markdown</button></div></header>
          <div class="api-detail-scroll">
            <section class="api-summary"><p>{{ selected.description || "暂无接口说明。" }}</p><div><span v-for="item in selected.tags" :key="item">{{ item }}</span><span v-if="selected.operationId">{{ selected.operationId }}</span><span v-if="selected.deprecated" class="danger">已弃用</span></div></section>
            <section><h3>请求参数</h3><div class="api-table-wrap"><table><thead><tr><th>名称</th><th>位置</th><th>类型</th><th>必填</th><th>说明</th><th>示例</th></tr></thead><tbody><tr v-for="(item,index) in parameters" :key="index"><td><code>{{ asText(item.name) }}</code></td><td>{{ asText(item.in) }}</td><td>{{ schemaType(asObject(item.schema)) }}</td><td>{{ asBoolean(item.required) ? '是' : '否' }}</td><td>{{ asText(item.description) || '—' }}</td><td><pre>{{ parameterExample(item) || '—' }}</pre></td></tr><tr v-if="!parameters.length"><td colspan="6">无请求参数</td></tr></tbody></table></div></section>
            <section><h3>请求体 <small v-if="asBoolean(requestBody.required)">必填</small></h3><article v-for="item in requestContent" :key="item.contentType" class="api-schema-card"><b>{{ item.contentType }}</b><div class="api-table-wrap"><table><thead><tr><th>字段</th><th>类型</th><th>必填</th><th>说明</th><th>示例</th></tr></thead><tbody><tr v-for="row in schemaRows(item.body.schema)" :key="row.name"><td><code>{{ row.name }}</code></td><td>{{ row.type }}</td><td>{{ row.required ? '是':'否' }}</td><td>{{ row.description || '—' }}</td><td><pre>{{ row.example || '—' }}</pre></td></tr><tr v-if="!schemaRows(item.body.schema).length"><td colspan="5">未声明字段结构</td></tr></tbody></table></div><details v-if="item.body.example"><summary>请求示例</summary><pre>{{ pretty(item.body.example) }}</pre></details></article><p v-if="!requestContent.length">无请求体。</p></section>
            <section><h3>响应</h3><article v-for="item in responses" :key="item.status" class="api-response-card"><header><b>{{ item.status }}</b><span>{{ asText(item.response.description) || '未填写说明' }}</span></header><div v-for="content in responseContents(item.response)" :key="content.contentType" class="api-schema-card"><b>{{ content.contentType }}</b><div class="api-table-wrap"><table><thead><tr><th>字段</th><th>类型</th><th>必填</th><th>说明</th><th>示例</th></tr></thead><tbody><tr v-for="row in schemaRows(content.body.schema)" :key="row.name"><td><code>{{ row.name }}</code></td><td>{{ row.type }}</td><td>{{ row.required ? '是':'否' }}</td><td>{{ row.description || '—' }}</td><td><pre>{{ row.example || '—' }}</pre></td></tr><tr v-if="!schemaRows(content.body.schema).length"><td colspan="5">未声明字段结构</td></tr></tbody></table></div><details v-if="content.body.example || content.body.examples"><summary>响应示例</summary><pre>{{ pretty(content.body.example || content.body.examples) }}</pre></details></div></article><p v-if="!responses.length">暂无响应定义。</p></section>
            <section><h3>鉴权</h3><p>{{ securityNames.length ? securityNames.join('、') : '未声明鉴权方案。' }}</p></section>
            <section v-if="warnings.length" class="api-warnings"><h3>解析提示</h3><p v-for="item in warnings" :key="item">{{ item }}</p></section>
          </div>
        </template>
        <div v-else class="api-detail-empty"><b>选择一个接口查看详情</b><p>复制文档时会在后端统一生成 Markdown，并遮盖敏感示例值。</p></div>
      </article>
    </section>

    <div v-if="configOpen" class="editor-backdrop" @click.self="configOpen=false"><aside class="task-editor api-source-editor"><header><div><h2>{{ config.id ? '编辑 Apifox 关联' : '关联 Apifox 项目' }}</h2><p>一个规范项目只关联一个 Apifox 项目</p></div><button class="icon-button" @click="configOpen=false">×</button></header><label>规范项目<select v-model="config.projectProfileId" :disabled="Boolean(config.id)"><option value="">请选择项目</option><option v-for="item in configurableProfiles" :key="item.id" :value="item.id">{{ item.displayName }} · {{ item.repositoryPath || '未关联仓库' }}</option></select></label><label>Apifox 项目 ID<input v-model="config.externalProjectId" autocomplete="off" placeholder="在 Apifox 项目设置 → 基本设置中复制"></label><p>这里不保存令牌。全局令牌请在“设置 → 外部服务”中统一管理。</p><footer><span></span><button class="button secondary" @click="configOpen=false">取消</button><button class="button primary" :disabled="loading || !config.projectProfileId || !config.externalProjectId.trim()" @click="persistSource">保存关联</button></footer></aside></div>
  </div>
</template>

<style scoped>
.api-onboarding{margin-bottom:12px;padding:18px;display:flex;align-items:center;gap:14px}.api-onboarding p{flex:1;margin:0;color:var(--muted)}
.api-docs-layout{height:calc(100vh - 164px);min-height:650px;display:grid;grid-template-columns:250px 370px minmax(460px,1fr);overflow:hidden}.api-source-panel,.api-endpoint-panel{border-right:1px solid var(--line);min-width:0;display:flex;flex-direction:column}.api-source-panel>header,.api-endpoint-panel>header{min-height:67px;padding:11px 14px;border-bottom:1px solid var(--line);display:flex;align-items:center;justify-content:space-between;gap:10px}.api-source-panel header div,.api-endpoint-panel header div{display:grid;gap:5px}.api-source-panel header small,.api-endpoint-panel header small{color:var(--muted);font-size:9px}.api-source-list,.api-endpoint-list{overflow:auto;flex:1}.api-source-list>button{width:100%;border:0;border-bottom:1px solid var(--line);background:transparent;color:inherit;padding:12px;display:flex;text-align:left;align-items:center;gap:8px}.api-source-list>button:hover,.api-source-list>button.active{background:var(--primary-soft)}.api-source-list button span{min-width:0;flex:1;display:grid;gap:5px}.api-source-list button small{white-space:nowrap;overflow:hidden;text-overflow:ellipsis;color:var(--muted)}.api-source-list button em{font-style:normal;font-size:9px;color:var(--muted)}.api-source-list button em.ready{color:var(--success)}.api-source-list button em.stale,.api-source-list button em.error{color:var(--danger)}.api-source-list>p,.api-endpoint-list>p{padding:24px 14px;color:var(--muted);line-height:1.7}.api-source-panel>footer{padding:10px;border-top:1px solid var(--line);display:flex;justify-content:space-between}
.api-cache-warning{margin:10px 12px 0;padding:9px;border-radius:7px;background:color-mix(in srgb,var(--warning) 10%,var(--surface));color:var(--warning);display:grid;gap:4px;font-size:9px}.api-filters{padding:10px 12px;border-bottom:1px solid var(--line);display:grid;grid-template-columns:1fr 1fr 1fr;gap:7px}.api-filters label{grid-column:1/4;height:35px;border:1px solid var(--line);border-radius:7px;display:flex;align-items:center;gap:7px;padding:0 9px;color:var(--muted)}.api-filters input{flex:1;min-width:0;border:0;outline:0;background:transparent;color:var(--text)}.api-filters select{height:32px;border:1px solid var(--line);border-radius:7px;background:var(--surface-2);color:var(--text);font-size:9px}.api-endpoint-list>button{width:100%;min-height:72px;border:0;border-bottom:1px solid var(--line);background:transparent;color:inherit;padding:10px 12px;display:flex;align-items:flex-start;gap:9px;text-align:left}.api-endpoint-list>button:hover,.api-endpoint-list>button.active{background:var(--primary-soft)}.api-endpoint-list button>span:nth-child(2){min-width:0;flex:1;display:grid;gap:4px}.api-endpoint-list code,.api-endpoint-list small{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.api-endpoint-list small{color:var(--muted);font-size:9px}.api-endpoint-list em{font-size:8px;color:var(--danger);font-style:normal}
.api-method{min-width:47px;padding:4px 5px;border-radius:5px;background:var(--surface-2);color:var(--muted);font-size:8px;font-weight:800;text-align:center}.api-method.get{background:color-mix(in srgb,#16845b 13%,var(--surface));color:#16845b}.api-method.post{background:color-mix(in srgb,#2563eb 13%,var(--surface));color:#2563eb}.api-method.put,.api-method.patch{background:color-mix(in srgb,#b7791f 14%,var(--surface));color:#b7791f}.api-method.delete{background:color-mix(in srgb,var(--danger) 13%,var(--surface));color:var(--danger)}.api-method.large{font-size:10px;padding:7px;min-width:56px}
.api-detail-panel{min-width:0;display:flex;flex-direction:column}.api-detail-panel>header{min-height:75px;padding:11px 15px;border-bottom:1px solid var(--line);display:flex;align-items:center;justify-content:space-between;gap:10px}.api-detail-panel>header>div{display:flex;align-items:center;gap:10px}.api-detail-panel h2{margin:0 0 5px;font-size:16px}.api-detail-panel header code{color:var(--muted)}.api-detail-scroll{overflow:auto;padding:15px}.api-detail-scroll>section{margin-bottom:20px}.api-detail-scroll h3{font-size:12px;margin:0 0 9px}.api-detail-scroll h3 small{color:var(--danger);font-size:8px}.api-summary p{line-height:1.7}.api-summary div{display:flex;gap:6px;flex-wrap:wrap}.api-summary span{padding:4px 7px;border-radius:5px;background:var(--surface-2);font-size:8px;color:var(--muted)}.api-summary span.danger{color:var(--danger)}.api-table-wrap{overflow:auto;border:1px solid var(--line);border-radius:7px}.api-table-wrap table{width:100%;border-collapse:collapse;font-size:9px}.api-table-wrap th,.api-table-wrap td{padding:7px 8px;border-bottom:1px solid var(--line);text-align:left;vertical-align:top}.api-table-wrap th{background:var(--surface-2);color:var(--muted);white-space:nowrap}.api-table-wrap pre{margin:0;max-width:220px;white-space:pre-wrap;word-break:break-all;font:inherit}.api-schema-card{display:grid;gap:8px;margin:8px 0}.api-schema-card details{border:1px solid var(--line);border-radius:7px;padding:8px}.api-schema-card details pre{white-space:pre-wrap;word-break:break-word}.api-response-card{border:1px solid var(--line);border-radius:8px;margin:8px 0;padding:10px}.api-response-card>header{display:flex;gap:9px;margin-bottom:8px}.api-response-card>header b{color:var(--primary)}.api-response-card>header span{color:var(--muted)}.api-warnings{padding:10px;border-radius:8px;background:color-mix(in srgb,var(--warning) 10%,var(--surface))}.api-warnings p{color:var(--warning)}.api-detail-empty{margin:auto;text-align:center;color:var(--muted)}.api-detail-empty b{color:var(--text);font-size:15px}.api-source-editor>p{padding:0 18px;color:var(--muted)}
@media(max-width:1200px){.api-docs-layout{grid-template-columns:220px 330px minmax(420px,1fr)}}
.api-docs-layout{grid-template-columns:220px 320px minmax(0,1fr)}
.api-detail-panel>header{flex-wrap:wrap}
.api-detail-empty{padding:20px}
@media(max-width:1500px){.api-docs-layout{grid-template-columns:200px 300px minmax(0,1fr)}}
</style>
