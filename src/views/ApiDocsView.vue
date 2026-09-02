<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { confirmAction } from "../utils/confirm";
import {
  clearApiTestToken,
  executeApiEndpointTest,
  getApiCodeTemplate,
  getApiEndpoint,
  getApiTagExport,
  getApiTestConfig,
  getApifoxCredentialStatus,
  isTauriRuntime,
  listApiEndpoints,
  listApiSources,
  previewApiEndpointTest,
  removeApiSource,
  renderApiEndpointRequestCode,
  renderApiEndpointMarkdown,
  saveApiCodeTemplate,
  saveApiSource,
  saveApiTestConfig,
  syncAllApiSources,
  syncApiSource,
  type ApiEndpointDetail,
  type ApiEndpointSummary,
  type ApiCodeTemplate,
  type ApiSource,
  type ApiTagExport,
  type ApiTestPreview,
  type ApiTestConfigUpdate,
  type ApiTestResult,
  type ApifoxCredentialStatus,
} from "../services/backend";

type JsonObject = Record<string, unknown>;
interface SchemaRow { name:string; type:string; required:boolean; description:string; example:string }
interface ApiTreeNode { key:string; name:string; children:ApiTreeNode[]; endpoints:ApiEndpointSummary[]; count:number }
interface ApiTreeRow { kind:"group"|"endpoint"; key:string; name:string; depth:number; count:number; expanded:boolean; tagPath?:string; item?:ApiEndpointSummary }

const route = useRoute();
const router = useRouter();
const credential = ref<ApifoxCredentialStatus>({ configured:false, source:"未配置" });
const sources = ref<ApiSource[]>([]);
const selectedSourceId = ref("");
const endpoints = ref<ApiEndpointSummary[]>([]);
const selected = ref<ApiEndpointDetail | null>(null);
const loading = ref(false);
const detailLoading = ref(false);
const syncingSourceId = ref("");
const syncingAll = ref(false);
const message = ref("");
const error = ref("");
const query = ref("");
const sourcePanelCollapsed = ref(false);
const expandedTreeKeys = ref(new Set<string>());
const configOpen = ref(false);
const settingsLoading = ref(false);
const config = reactive({ id:"", externalProjectId:"", apifoxProjectName:"" });
const testTokenConfigured = ref(false);
const testConfig = reactive<ApiTestConfigUpdate>({ sourceId:"", baseUrl:"", tokenHeader:"Authorization", token:"" });
const testingEndpointId = ref("");
const testResult = ref<ApiTestResult|null>(null);
const testPreviewOpen = ref(false);
const testPreviewLoading = ref(false);
const testPreview = ref<ApiTestPreview|null>(null);
const testPreviewUrl = ref("");
const testPreviewBody = ref("");
const testPreviewConfirmed = ref(false);
const testPreviewError = ref("");
const tagExportOpen = ref(false);
const tagExportLoading = ref(false);
const tagExportVersion = ref<"3.0"|"3.1">("3.0");
const tagExport = reactive<ApiTagExport>({ sourceId:"", tagPath:"", openapiUrl:"", endpointCount:0, available:false });
const codeTemplate = reactive<ApiCodeTemplate>({ sourceId:"", client:"request", functionPrefix:"_", importPath:"", includeImport:false, typescript:false });

function asObject(value:unknown):JsonObject { return value && typeof value === "object" && !Array.isArray(value) ? value as JsonObject : {}; }
function asArray(value:unknown):unknown[] { return Array.isArray(value) ? value : []; }
function asText(value:unknown):string { return typeof value === "string" ? value : ""; }
function asBoolean(value:unknown):boolean { return value === true; }
function pretty(value:unknown):string { return value === undefined ? "" : typeof value === "string" ? value : JSON.stringify(value,null,2); }
const activeSource = computed(() => sources.value.find(item=>item.id===selectedSourceId.value) || null);
const tagExportUrl = computed(() => tagExport.openapiUrl.replace(/([?&]version=)[^&]+/,`$1${tagExportVersion.value}`));
const filtered = computed(() => {
  const needle=query.value.trim().toLowerCase();
  return endpoints.value.filter(item => !needle || `${item.title} ${item.path} ${item.description} ${item.operationId} ${item.tags.join(" ")}`.toLowerCase().includes(needle));
});

function endpointFolders(item:ApiEndpointSummary):string[] {
  const folder=item.tags[0]?.trim() || "未分组";
  return folder.split(/[\\/›>]+/).map(value=>value.trim()).filter(Boolean);
}
function endpointFolderKeys(item:ApiEndpointSummary):string[] {
  const keys:string[]=[];
  endpointFolders(item).reduce((prefix,name) => {
    const key=prefix ? `${prefix}/${name}` : name;
    keys.push(key);
    return key;
  },"");
  return keys;
}
function buildEndpointTree(items:ApiEndpointSummary[]):ApiTreeNode[] {
  const roots:ApiTreeNode[]=[];
  for(const item of items) {
    let nodes=roots;
    let prefix="";
    let leaf:ApiTreeNode|undefined;
    for(const name of endpointFolders(item)) {
      const key=prefix ? `${prefix}/${name}` : name;
      let node=nodes.find(value=>value.key===key);
      if(!node) { node={key,name,children:[],endpoints:[],count:0};nodes.push(node); }
      leaf=node;
      nodes=node.children;
      prefix=key;
    }
    leaf?.endpoints.push(item);
  }
  const finalize=(nodes:ApiTreeNode[]):number => nodes.reduce((total,node) => {
    node.children.sort((a,b)=>a.name.localeCompare(b.name,"zh-CN"));
    node.endpoints.sort((a,b)=>a.title.localeCompare(b.title,"zh-CN") || a.path.localeCompare(b.path));
    node.count=node.endpoints.length+finalize(node.children);
    return total+node.count;
  },0);
  roots.sort((a,b)=>a.name.localeCompare(b.name,"zh-CN"));
  finalize(roots);
  return roots;
}
const endpointTree = computed(() => buildEndpointTree(filtered.value));
const forceExpandTree = computed(() => Boolean(query.value.trim()));
const treeRows = computed<ApiTreeRow[]>(() => {
  const rows:ApiTreeRow[]=[];
  const append=(nodes:ApiTreeNode[],depth:number) => {
    for(const node of nodes) {
      const expanded=forceExpandTree.value || expandedTreeKeys.value.has(node.key);
      rows.push({kind:"group",key:`group:${node.key}`,name:node.name,depth,count:node.count,expanded,tagPath:node.key});
      if(!expanded) continue;
      append(node.children,depth+1);
      for(const item of node.endpoints) rows.push({kind:"endpoint",key:`endpoint:${item.id}`,name:item.title,depth:depth+1,count:0,expanded:false,item});
    }
  };
  append(endpointTree.value,0);
  return rows;
});
function expandEndpointPath(item?:ApiEndpointSummary|null) {
  if(!item) return;
  const next=new Set(expandedTreeKeys.value);
  endpointFolderKeys(item).forEach(key=>next.add(key));
  expandedTreeKeys.value=next;
}
function toggleTreeGroup(row:ApiTreeRow) {
  const key=row.key.replace(/^group:/,"");
  const next=new Set(expandedTreeKeys.value);
  if(next.has(key)) next.delete(key); else next.add(key);
  expandedTreeKeys.value=next;
}
function selectTreeEndpoint(item?:ApiEndpointSummary) { if(item) void selectEndpoint(item); }
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
  expandedTreeKeys.value=new Set();
  expandEndpointPath(endpoints.value.find(item=>item.id===selected.value?.id) || endpoints.value[0]);
  if(selected.value) await router.replace({query:{source:sourceId,endpoint:selected.value.id}});
}

async function load() {
  if(!isTauriRuntime()) return;
  loading.value=true; error.value="";
  try {
    [credential.value,sources.value]=await Promise.all([getApifoxCredentialStatus(),listApiSources()]);
    const requestedEndpoint=String(route.query.endpoint || "");
    let requestedDetail:ApiEndpointDetail|null=null;
    if(requestedEndpoint) { try { requestedDetail=await getApiEndpoint(requestedEndpoint); } catch { /* 已删除的旧链接回退到项目列表。 */ } }
    const sourceId=String(route.query.source || requestedDetail?.sourceId || "");
    const legacyProjectId=String(route.query.project || "");
    const target=sources.value.find(item=>item.id===requestedDetail?.sourceId)
      || sources.value.find(item=>item.id===sourceId)
      || sources.value.find(item=>legacyProjectId && item.projectProfileId===legacyProjectId)
      || sources.value.find(item=>item.id===selectedSourceId.value)
      || sources.value[0];
    selectedSourceId.value=target?.id || "";
    if(target) await loadEndpoints(target.id,requestedDetail?.id || requestedEndpoint);
  } catch(cause) { error.value=String(cause); }
  finally { loading.value=false; }
}

async function selectSource(item:ApiSource) {
  selectedSourceId.value=item.id; query.value="";error.value="";testResult.value=null;
  detailLoading.value=true;
  try { await loadEndpoints(item.id); }
  catch(cause){error.value=String(cause);}finally{detailLoading.value=false;}
}
async function selectEndpoint(item:ApiEndpointSummary) {
  error.value="";testResult.value=null;detailLoading.value=true;
  try { expandEndpointPath(item);selected.value=await getApiEndpoint(item.id);await router.replace({query:{source:selected.value.sourceId,endpoint:item.id}}); }
  catch(cause){error.value=String(cause);}finally{detailLoading.value=false;}
}
async function openProjectSettings(source?:ApiSource|null) {
  const target=source || null;
  configOpen.value=true;settingsLoading.value=true;error.value="";message.value="";
  config.id=target?.id || "";
  config.externalProjectId=target?.externalProjectId || "";
  config.apifoxProjectName=target?.apifoxProjectName || target?.documentTitle || "";
  Object.assign(testConfig,{sourceId:target?.id || "",baseUrl:"",tokenHeader:"Authorization",token:""});
  Object.assign(codeTemplate,{sourceId:target?.id || "",client:"request",functionPrefix:"_",importPath:"",includeImport:false,typescript:false});
  testTokenConfigured.value=false;
  if(!target) { settingsLoading.value=false;return; }
  try {
    const [requestConfig,template]=await Promise.all([getApiTestConfig(target.id),getApiCodeTemplate(target.id)]);
    Object.assign(testConfig,{sourceId:requestConfig.sourceId,baseUrl:requestConfig.baseUrl,tokenHeader:requestConfig.tokenHeader || "Authorization",token:""});
    testTokenConfigured.value=requestConfig.tokenConfigured;
    Object.assign(codeTemplate,template);
  } catch(cause) { error.value=String(cause); }
  finally { settingsLoading.value=false; }
}
async function persistProjectSettings() {
  settingsLoading.value=true;error.value="";message.value="";
  try {
    const saved=await saveApiSource({id:config.id,externalProjectId:config.externalProjectId,apifoxProjectName:config.apifoxProjectName});
    await Promise.all([
      saveApiTestConfig({...testConfig,sourceId:saved.id}),
      saveApiCodeTemplate({...codeTemplate,sourceId:saved.id}),
    ]);
    configOpen.value=false;message.value=`Apifox 项目“${saved.apifoxProjectName}”设置已保存。`;await load();
    await selectSource(sources.value.find(item=>item.id===saved.id) || saved);
  } catch(cause){error.value=String(cause);}finally{settingsLoading.value=false;}
}
async function removeSource(item:ApiSource) {
  if(!await confirmAction({ title:"删除 Apifox 项目", message:`确定删除 Apifox 项目“${item.apifoxProjectName || item.externalProjectId}”及其本地接口缓存吗？Apifox 原项目不会删除。`, confirmText:"删除项目", tone:"danger" })) return;
  loading.value=true;error.value="";
  try { await removeApiSource(item.id);configOpen.value=false;message.value="Apifox 项目和本地接口缓存已移除。";selected.value=null;await load(); }
  catch(cause){error.value=String(cause);}finally{loading.value=false;}
}
async function syncOne(item:ApiSource) {
  syncingSourceId.value=item.id;error.value="";message.value="";
  try { const result=await syncApiSource(item.id);message.value=`${item.apifoxProjectName || item.externalProjectId} 同步完成：新增 ${result.added}、更新 ${result.updated}、移除 ${result.removed}，共 ${result.total} 个接口。`;await load(); }
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
async function copyRequestCode() {
  if(!selected.value) return;
  try { const value=await renderApiEndpointRequestCode(selected.value.id);await navigator.clipboard.writeText(value);message.value="接口请求函数代码已复制。"; }
  catch(cause){error.value=String(cause);}
}
async function copyPath() {
  if(!selected.value) return;
  try { await navigator.clipboard.writeText(selected.value.path);message.value="接口路径已复制。"; }
  catch(cause){error.value=String(cause);}
}
async function clearTestToken() {
  if(!testConfig.sourceId) return;
  settingsLoading.value=true;error.value="";
  try { const value=await clearApiTestToken(testConfig.sourceId);testTokenConfigured.value=value.tokenConfigured;testConfig.token="";message.value="接口测试 Token 已清除。"; }
  catch(cause){error.value=String(cause);}finally{settingsLoading.value=false;}
}
async function runEndpointTest() {
  if(!selected.value) return;
  testPreviewLoading.value=true;testResult.value=null;error.value="";message.value="";testPreviewConfirmed.value=false;testPreviewError.value="";
  try {
    const value=await previewApiEndpointTest(selected.value.id);
    testPreview.value=value;testPreviewUrl.value=value.url;testPreviewBody.value=value.body===null ? "" : JSON.stringify(value.body,null,2);testPreviewOpen.value=true;
  }
  catch(cause){error.value=String(cause);if(error.value.includes("请求基地址")) await openProjectSettings(activeSource.value);}
  finally{testPreviewLoading.value=false;}
}
async function sendEndpointTest() {
  if(!testPreview.value) return;
  let body:unknown=null;
  if(testPreview.value.body!==null) {
    try { body=testPreviewBody.value.trim() ? JSON.parse(testPreviewBody.value) : null; }
    catch { testPreviewError.value="请求体不是有效的 JSON，请修改后再发送。";return; }
  }
  testingEndpointId.value=testPreview.value.endpointId;error.value="";message.value="";testPreviewError.value="";
  try {
    testResult.value=await executeApiEndpointTest({endpointId:testPreview.value.endpointId,url:testPreviewUrl.value.trim(),body,confirmed:testPreviewConfirmed.value});
    testPreviewOpen.value=false;message.value=`接口测试完成：HTTP ${testResult.value.status}，耗时 ${testResult.value.elapsedMs} ms。`;
  } catch(cause){testPreviewError.value=String(cause);}
  finally{testingEndpointId.value="";}
}
async function openTagExport(row:ApiTreeRow) {
  if(!activeSource.value || !row.tagPath) return;
  tagExportLoading.value=true;error.value="";
  try {
    const value=await getApiTagExport(activeSource.value.id,row.tagPath);
    Object.assign(tagExport,value);tagExportVersion.value="3.0";tagExportOpen.value=true;
  } catch(cause){error.value=String(cause);}finally{tagExportLoading.value=false;}
}
async function copyTagExportUrl() {
  if(!tagExportUrl.value) return;
  try { await navigator.clipboard.writeText(tagExportUrl.value);message.value=`“${tagExport.tagPath}”整个标签的 OpenAPI ${tagExportVersion.value} URL 已复制。`; }
  catch(cause){error.value=String(cause);}
}
function openTagExportUrl() {
  if(tagExportUrl.value) window.open(tagExportUrl.value,"_blank","noopener,noreferrer");
}
onMounted(load);
</script>

<template>
  <div class="view api-docs-view">
    <header class="page-header"><div><h1>接口文档中心</h1><p>管理 Apifox 项目及其 OpenAPI 文档，统一查询、复制和复用</p></div><div><RouterLink v-if="!credential.configured" class="button secondary link-button" to="/settings">配置 Apifox 令牌</RouterLink><button class="button secondary" :disabled="syncingAll || !sources.length || !credential.configured" @click="syncAll">{{ syncingAll ? "同步中…" : "同步全部" }}</button><button v-if="activeSource" class="button secondary" @click="openProjectSettings(activeSource)">项目设置</button><button class="button primary" @click="openProjectSettings(null)">＋ 新增 Apifox 项目</button></div></header>
    <div v-if="message || error" class="scan-message" :class="{error:Boolean(error)}">{{ error || message }}</div>
    <section v-if="!credential.configured" class="panel api-onboarding"><b>先配置 Apifox API 访问令牌</b><p>令牌只保存在 Windows 凭据库，接口文档缓存才会写入本地 SQLite。</p><RouterLink class="button primary link-button" to="/settings">前往设置</RouterLink></section>
    <section class="panel api-docs-layout" :class="{'source-collapsed':sourcePanelCollapsed}">
      <aside class="api-source-panel">
        <header><div><b>Apifox 项目</b><small>{{ sources.length }} 个项目</small></div><button class="text-button" @click="sourcePanelCollapsed=true">收起</button></header>
        <div class="api-source-list">
          <button v-for="item in sources" :key="item.id" :class="{active:item.id===selectedSourceId}" @click="selectSource(item)"><span><b>{{ item.apifoxProjectName || item.documentTitle || `Apifox ${item.externalProjectId}` }}</b><small>项目 ID：{{ item.externalProjectId }}</small><em :class="item.syncStatus">{{ statusText(item.syncStatus) }} · {{ item.endpointCount }} 个</em></span></button>
          <p v-if="!sources.length">尚未添加 Apifox 项目，请点击右上角“新增 Apifox 项目”。</p>
        </div>
      </aside>

      <main class="api-endpoint-panel">
        <header><div class="api-endpoint-heading"><button v-if="sourcePanelCollapsed" class="button secondary small" @click="sourcePanelCollapsed=false">项目列表</button><span><b>{{ activeSource?.apifoxProjectName || activeSource?.documentTitle || "接口列表" }}</b><small>{{ activeSource ? `更新于 ${formatTime(activeSource.lastSyncedAt)}` : '请选择 Apifox 项目' }}</small></span></div><div v-if="activeSource" class="api-source-actions"><button class="button secondary small" :disabled="syncingSourceId===activeSource.id || !credential.configured" @click="syncOne(activeSource)">{{ syncingSourceId===activeSource.id ? "同步中…" : "↻ 同步" }}</button></div></header>
        <div v-if="activeSource?.lastError" class="api-cache-warning"><b>{{ activeSource.syncStatus==='stale' ? '正在使用上次成功缓存' : '同步未完成' }}</b><span>{{ activeSource.lastError }}</span></div>
        <div class="api-filters"><label>⌕<input v-model="query" placeholder="搜索名称、路径、描述或 Operation ID"></label></div>
        <div class="api-endpoint-list" aria-label="接口目录树">
          <template v-for="row in treeRows" :key="row.key">
            <div v-if="row.kind==='group'" class="api-tree-group" :style="{paddingLeft:`${12+row.depth*16}px`}"><button class="api-tree-toggle" :aria-expanded="row.expanded" @click="toggleTreeGroup(row)"><span class="disclosure-icon tree" aria-hidden="true"></span><b>{{ row.name }}</b><em>{{ row.count }}</em></button><button class="api-tag-export-trigger" title="导出整个标签的 OpenAPI URL" :disabled="tagExportLoading" @click="openTagExport(row)">URL</button></div>
            <button v-else class="api-tree-endpoint" :class="{active:row.item?.id===selected?.id}" :style="{paddingLeft:`${15+row.depth*16}px`}" @click="selectTreeEndpoint(row.item)"><span class="api-method" :class="row.item?.method.toLowerCase()">{{ row.item?.method }}</span><span class="api-tree-copy"><b>{{ row.item?.title }}</b><code>{{ row.item?.path }}</code></span><em v-if="row.item?.deprecated">弃用</em></button>
          </template>
          <p v-if="activeSource && !filtered.length">{{ endpoints.length ? "没有符合筛选条件的接口。" : "尚无缓存，请点击同步。" }}</p><p v-if="!activeSource">先从左侧选择或新增一个 Apifox 项目。</p>
        </div>
      </main>

      <article class="api-detail-panel">
        <div v-if="detailLoading" class="api-detail-loading" role="status"><i></i><b>正在加载接口详情</b><span>请求参数、请求体和响应定义加载完成后会显示在这里。</span></div>
        <template v-else-if="selected">
          <header><div><span class="api-method large" :class="selected.method.toLowerCase()">{{ selected.method }}</span><div><h2>{{ selected.title }}</h2><button class="api-path-copy" title="复制接口路径" @click="copyPath"><code>{{ selected.path }}</code><span>复制</span></button></div></div><div><button class="button secondary small" :disabled="testingEndpointId===selected.id || testPreviewLoading" @click="runEndpointTest">{{ testPreviewLoading ? "生成预览…" : testingEndpointId===selected.id ? "测试中…" : "接口测试" }}</button><button class="button secondary small" @click="copyRequestCode">复制接口代码</button><button class="button primary small" @click="copyMarkdown">复制 Markdown</button></div></header>
          <div class="api-detail-scroll">
            <section class="api-summary"><p>{{ selected.description || "暂无接口说明。" }}</p><div><span v-for="item in selected.tags" :key="item">{{ item }}</span><span v-if="selected.operationId">{{ selected.operationId }}</span><span v-if="selected.deprecated" class="danger">已弃用</span></div></section>
            <section v-if="testResult" class="api-test-result" :class="{success:testResult.success,error:!testResult.success}"><header><div><h3>接口测试结果</h3><p><b>HTTP {{ testResult.status }}</b> {{ testResult.statusText }} · {{ testResult.elapsedMs }} ms</p></div><span>{{ testResult.method }} {{ testResult.url }}</span></header><div class="api-test-result-grid"><article><b>自动生成的请求数据</b><pre>{{ pretty(testResult.requestData) }}</pre></article><article><b>实际响应数据</b><small v-if="testResult.contentType">{{ testResult.contentType }}</small><pre>{{ testResult.responseData===null ? '（空响应）' : pretty(testResult.responseData) }}</pre><p v-if="testResult.truncated">响应超过 1 MB，当前只展示前 1 MB。</p></article></div></section>
            <section><h3>请求参数</h3><div class="api-table-wrap"><table><thead><tr><th>名称</th><th>位置</th><th>类型</th><th>必填</th><th>说明</th><th>示例</th></tr></thead><tbody><tr v-for="(item,index) in parameters" :key="index"><td><code>{{ asText(item.name) }}</code></td><td>{{ asText(item.in) }}</td><td>{{ schemaType(asObject(item.schema)) }}</td><td>{{ asBoolean(item.required) ? '是' : '否' }}</td><td>{{ asText(item.description) || '—' }}</td><td><pre>{{ parameterExample(item) || '—' }}</pre></td></tr><tr v-if="!parameters.length"><td colspan="6">无请求参数</td></tr></tbody></table></div></section>
            <section><h3>请求体 <small v-if="asBoolean(requestBody.required)">必填</small></h3><article v-for="item in requestContent" :key="item.contentType" class="api-schema-card"><b>{{ item.contentType }}</b><div class="api-table-wrap"><table><thead><tr><th>字段</th><th>类型</th><th>必填</th><th>说明</th><th>示例</th></tr></thead><tbody><tr v-for="row in schemaRows(item.body.schema)" :key="row.name"><td><code>{{ row.name }}</code></td><td>{{ row.type }}</td><td>{{ row.required ? '是':'否' }}</td><td>{{ row.description || '—' }}</td><td><pre>{{ row.example || '—' }}</pre></td></tr><tr v-if="!schemaRows(item.body.schema).length"><td colspan="5">未声明字段结构</td></tr></tbody></table></div><details v-if="item.body.example"><summary>请求示例</summary><pre>{{ pretty(item.body.example) }}</pre></details></article><p v-if="!requestContent.length">无请求体。</p></section>
            <section><h3>响应</h3><article v-for="item in responses" :key="item.status" class="api-response-card"><header><b>{{ item.status }}</b><span>{{ asText(item.response.description) || '未填写说明' }}</span></header><div v-for="content in responseContents(item.response)" :key="content.contentType" class="api-schema-card"><b>{{ content.contentType }}</b><div class="api-table-wrap"><table><thead><tr><th>字段</th><th>类型</th><th>必填</th><th>说明</th><th>示例</th></tr></thead><tbody><tr v-for="row in schemaRows(content.body.schema)" :key="row.name"><td><code>{{ row.name }}</code></td><td>{{ row.type }}</td><td>{{ row.required ? '是':'否' }}</td><td>{{ row.description || '—' }}</td><td><pre>{{ row.example || '—' }}</pre></td></tr><tr v-if="!schemaRows(content.body.schema).length"><td colspan="5">未声明字段结构</td></tr></tbody></table></div><details v-if="content.body.example || content.body.examples"><summary>响应示例</summary><pre>{{ pretty(content.body.example || content.body.examples) }}</pre></details></div></article><p v-if="!responses.length">暂无响应定义。</p></section>
            <section><h3>鉴权</h3><p>{{ securityNames.length ? securityNames.join('、') : '未声明鉴权方案。' }}</p></section>
            <section v-if="warnings.length" class="api-warnings"><h3>解析提示</h3><p v-for="item in warnings" :key="item">{{ item }}</p></section>
          </div>
        </template>
        <div v-else class="api-detail-empty"><b>选择一个接口查看详情</b><p>可复制接口路径、请求代码，或使用自动生成的数据执行接口测试。</p></div>
      </article>
    </section>

    <div v-if="configOpen" class="editor-backdrop" @click.self="configOpen=false"><aside class="task-editor api-project-settings-editor"><header><div><h2>{{ config.id ? '项目设置' : '新增 Apifox 项目' }}</h2><p>统一管理 Apifox 项目、接口测试和复制代码配置</p></div><button class="icon-button" @click="configOpen=false">×</button></header><div class="api-project-settings-scroll"><section><h3>Apifox 项目</h3><div class="api-credential-status"><span>API 令牌：{{ credential.configured ? credential.source : '未配置' }}</span><RouterLink class="text-button" to="/settings">前往全局设置</RouterLink></div><label>Apifox 项目名称<input v-model="config.apifoxProjectName" autocomplete="off" placeholder="例如 client 接口项目"></label><label>Apifox 项目 ID<input v-model="config.externalProjectId" autocomplete="off" placeholder="在 Apifox 项目设置 → 基本设置中复制"></label></section><section><h3>接口测试</h3><label>请求基地址<input v-model="testConfig.baseUrl" autocomplete="off" placeholder="例如 https://api.example.com；留空使用 OpenAPI servers[0]"></label><label>Token 请求头名称<input v-model="testConfig.tokenHeader" autocomplete="off" placeholder="例如 Authorization 或 hlzt-token"></label><label>Token 值<input v-model="testConfig.token" type="password" autocomplete="new-password" :placeholder="testTokenConfigured ? '已保存；留空不会覆盖' : '例如 Bearer xxx 或原始 Token'"></label><p>Token {{ testTokenConfigured ? '已保存到 Windows 凭据库' : '尚未配置' }}，不会写入数据库、日志或请求预览。</p><button v-if="testTokenConfigured" class="text-button danger-text api-clear-token" :disabled="settingsLoading" @click="clearTestToken">清除当前项目 Token</button></section><section><h3>复制代码模板</h3><label>请求客户端<select v-model="codeTemplate.client"><option value="request">request 封装</option><option value="axios">axios</option><option value="uni-request">uni.request</option></select></label><label>函数名前缀<input v-model="codeTemplate.functionPrefix" autocomplete="off" placeholder="例如 _ 或 api"></label><label class="api-template-check"><input v-model="codeTemplate.typescript" type="checkbox"><span>生成 TypeScript 参数类型</span></label><label v-if="codeTemplate.client!=='uni-request'" class="api-template-check"><input v-model="codeTemplate.includeImport" type="checkbox"><span>在代码顶部包含 import</span></label><label v-if="codeTemplate.includeImport && codeTemplate.client!=='uni-request'">导入路径<input v-model="codeTemplate.importPath" autocomplete="off" :placeholder="codeTemplate.client==='axios' ? 'axios' : '@/utils/request'"></label><p>GET、DELETE 使用 params，POST、PUT、PATCH 使用 data；路径参数会自动加入函数参数。</p></section></div><footer><button v-if="activeSource && config.id===activeSource.id" class="text-button danger-text" :disabled="settingsLoading" @click="removeSource(activeSource)">删除当前项目</button><span v-else></span><button class="button secondary" @click="configOpen=false">取消</button><button class="button primary" :disabled="settingsLoading || !config.externalProjectId.trim() || !config.apifoxProjectName.trim() || !testConfig.tokenHeader.trim()" @click="persistProjectSettings">{{ settingsLoading ? '保存中…' : config.id ? '保存项目设置' : '新增项目' }}</button></footer></aside></div>
    <div v-if="testPreviewOpen && testPreview" class="editor-backdrop" @click.self="testPreviewOpen=false"><aside class="task-editor api-test-preview-editor"><header><div><h2>确认接口测试请求</h2><p>{{ testPreview.method }} 请求尚未发送，请先核对内容</p></div><button class="icon-button" @click="testPreviewOpen=false">×</button></header><div class="api-test-preview-scroll"><div class="api-preview-warning" :class="{danger:testPreview.requiresConfirmation}"><b>{{ testPreview.requiresConfirmation ? '可能修改真实数据' : '发送前确认' }}</b><span>{{ testPreview.warning }}</span></div><div v-if="testPreviewError" class="api-preview-error">{{ testPreviewError }}</div><label>最终请求地址<input v-model="testPreviewUrl" autocomplete="off"></label><section><b>请求头</b><div class="api-preview-headers"><span v-for="(value,name) in testPreview.headers" :key="name"><code>{{ name }}</code><em>{{ value }}</em></span><small v-if="!Object.keys(testPreview.headers).length">没有额外请求头</small></div></section><label v-if="testPreview.body!==null">请求体 JSON<textarea v-model="testPreviewBody" rows="10" spellcheck="false"></textarea></label><details><summary>查看自动生成的请求数据</summary><pre>{{ pretty(testPreview.requestData) }}</pre></details><label v-if="testPreview.requiresConfirmation" class="api-danger-confirm"><input v-model="testPreviewConfirmed" type="checkbox"><span>我已核对地址和请求体，并确认该请求可能修改真实业务数据</span></label></div><footer><span></span><button class="button secondary" @click="testPreviewOpen=false">取消</button><button class="button primary" :disabled="testingEndpointId===testPreview.endpointId || !testPreviewUrl.trim() || (testPreview.requiresConfirmation && !testPreviewConfirmed)" @click="sendEndpointTest">{{ testingEndpointId===testPreview.endpointId ? '发送中…' : `发送 ${testPreview.method} 请求` }}</button></footer></aside></div>
    <div v-if="tagExportOpen" class="editor-backdrop" @click.self="tagExportOpen=false"><aside class="task-editor api-tag-export-editor"><header><div><h2>标签 OpenAPI 导出</h2><p>“{{ tagExport.tagPath }}”及其子标签，共 {{ tagExport.endpointCount }} 个接口</p></div><button class="icon-button" @click="tagExportOpen=false">×</button></header><label>OpenAPI 版本<select v-model="tagExportVersion"><option value="3.0">OpenAPI 3.0</option><option value="3.1">OpenAPI 3.1</option></select></label><label>工作台本地 URL<input :value="tagExportUrl" readonly></label><p>该地址由工作台在 127.0.0.1 上只读提供，不依赖 Apifox 运行；工作台退出后地址会暂时不可访问。导出会保留公共 Schema 和鉴权定义，并遮盖敏感示例值。</p><footer><span></span><button class="button secondary" @click="openTagExportUrl">打开 URL</button><button class="button primary" @click="copyTagExportUrl">复制 URL</button></footer></aside></div>
  </div>
</template>

<style scoped>
.api-docs-view{height:calc(100vh - 118px);min-height:600px;display:flex;flex-direction:column;overflow:hidden}.api-docs-view>.page-header{flex:0 0 72px}
.api-onboarding{margin-bottom:12px;padding:18px;display:flex;align-items:center;gap:14px}.api-onboarding p{flex:1;margin:0;color:var(--muted)}
.api-docs-layout{flex:1;min-height:0;display:grid;grid-template-columns:220px 340px minmax(0,1fr);overflow:hidden;transition:grid-template-columns .18s ease}.api-docs-layout.source-collapsed{grid-template-columns:0 340px minmax(0,1fr)}.api-source-panel,.api-endpoint-panel,.api-detail-panel{min-width:0;min-height:0;display:flex;flex-direction:column}.api-source-panel{overflow:hidden}.source-collapsed .api-source-panel{visibility:hidden;border-right:0}.api-source-panel,.api-endpoint-panel{border-right:1px solid var(--line)}.api-source-panel>header,.api-endpoint-panel>header{min-height:67px;padding:11px 14px;border-bottom:1px solid var(--line);display:flex;align-items:center;justify-content:space-between;gap:10px}.api-source-panel header div,.api-endpoint-panel header div{display:grid;gap:5px}.api-source-panel header small,.api-endpoint-panel header small{color:var(--muted);font-size:10px}.api-endpoint-heading{display:flex!important;align-items:center;gap:9px!important}.api-endpoint-heading>span{display:grid;gap:5px}.api-source-list,.api-endpoint-list{min-height:0;overflow-y:auto;overflow-x:hidden;flex:1;scrollbar-gutter:stable}.api-source-list>button{width:100%;border:0;border-bottom:1px solid var(--line);background:transparent;color:inherit;padding:12px;display:flex;text-align:left;align-items:center;gap:8px}.api-source-list>button:hover,.api-source-list>button.active{background:var(--primary-soft)}.api-source-list button span{min-width:0;flex:1;display:grid;gap:5px}.api-source-list button small{white-space:nowrap;overflow:hidden;text-overflow:ellipsis;color:var(--muted)}.api-source-list button em{font-style:normal;font-size:10px;color:var(--muted)}.api-source-list button em.ready{color:var(--success)}.api-source-list button em.stale,.api-source-list button em.error{color:var(--danger)}.api-source-list>p,.api-endpoint-list>p{padding:24px 14px;color:var(--muted);line-height:1.7}
.api-cache-warning{margin:10px 12px 0;padding:9px;border-radius:7px;background:color-mix(in srgb,var(--warning) 10%,var(--surface));color:var(--warning);display:grid;gap:4px;font-size:10px}.api-filters{padding:10px 12px;border-bottom:1px solid var(--line)}.api-filters label{height:37px;border:1px solid var(--line);border-radius:7px;display:flex;align-items:center;gap:7px;padding:0 9px;color:var(--muted)}.api-filters input{flex:1;min-width:0;border:0;outline:0;background:transparent;color:var(--text)}.api-tree-group,.api-tree-endpoint{width:100%;border:0;border-bottom:1px solid var(--line);background:transparent;color:inherit;text-align:left}.api-tree-group{min-height:43px;display:flex;align-items:center;gap:7px;padding-top:8px;padding-right:12px;padding-bottom:8px}.api-tree-group:hover{background:color-mix(in srgb,var(--primary) 6%,transparent)}.api-tree-chevron{width:12px;color:var(--muted);font-size:15px;text-align:center}.api-tree-group b{min-width:0;flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-size:13px}.api-tree-group em{color:var(--muted);font-size:11px;font-style:normal}.api-tree-endpoint{min-height:62px;display:flex;align-items:flex-start;gap:9px;padding-top:9px;padding-right:12px;padding-bottom:9px}.api-tree-endpoint:hover,.api-tree-endpoint.active{background:var(--primary-soft)}.api-tree-copy{min-width:0;flex:1;display:grid;gap:5px}.api-tree-copy b,.api-tree-copy code{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.api-tree-copy b{font-size:12px}.api-tree-copy code{color:var(--muted);font-size:11px}.api-tree-endpoint>em{font-size:9px;color:var(--danger);font-style:normal}
.api-method{min-width:47px;padding:4px 5px;border-radius:5px;background:var(--surface-2);color:var(--muted);font-size:8px;font-weight:800;text-align:center}.api-method.get{background:color-mix(in srgb,#16845b 13%,var(--surface));color:#16845b}.api-method.post{background:color-mix(in srgb,#2563eb 13%,var(--surface));color:#2563eb}.api-method.put,.api-method.patch{background:color-mix(in srgb,#b7791f 14%,var(--surface));color:#b7791f}.api-method.delete{background:color-mix(in srgb,var(--danger) 13%,var(--surface));color:var(--danger)}.api-method.large{font-size:10px;padding:7px;min-width:56px}
.api-detail-panel>header{min-height:82px;padding:12px 16px;border-bottom:1px solid var(--line);display:flex;align-items:center;justify-content:space-between;gap:10px;flex-wrap:wrap}.api-detail-panel>header>div{display:flex;align-items:center;gap:10px}.api-detail-panel h2{margin:0 0 6px;font-size:19px}.api-detail-panel header code{color:var(--muted);font-size:13px}.api-detail-scroll{flex:1;min-height:0;overflow-y:auto;overflow-x:hidden;padding:18px;font-size:13px;line-height:1.65;scrollbar-gutter:stable}.api-detail-scroll>section{margin-bottom:24px}.api-detail-scroll h3{font-size:15px;margin:0 0 11px}.api-detail-scroll h3 small{color:var(--danger);font-size:11px}.api-summary p{font-size:14px;line-height:1.75}.api-summary div{display:flex;gap:7px;flex-wrap:wrap}.api-summary span{padding:5px 8px;border-radius:5px;background:var(--surface-2);font-size:11px;color:var(--muted)}.api-summary span.danger{color:var(--danger)}.api-table-wrap{max-width:100%;overflow:auto;border:1px solid var(--line);border-radius:7px}.api-table-wrap table{width:100%;border-collapse:collapse;font-size:12px}.api-table-wrap th,.api-table-wrap td{padding:9px 10px;border-bottom:1px solid var(--line);text-align:left;vertical-align:top;line-height:1.55}.api-table-wrap th{position:sticky;top:0;background:var(--surface-2);color:var(--muted);white-space:nowrap}.api-table-wrap code{font-size:12px}.api-table-wrap pre{margin:0;max-width:260px;white-space:pre-wrap;word-break:break-all;font:inherit}.api-schema-card{display:grid;gap:9px;margin:10px 0}.api-schema-card details{border:1px solid var(--line);border-radius:7px;padding:10px}.api-schema-card details pre{max-height:360px;overflow:auto;white-space:pre-wrap;word-break:break-word;font-size:12px;line-height:1.65}.api-response-card{border:1px solid var(--line);border-radius:8px;margin:10px 0;padding:12px}.api-response-card>header{display:flex;gap:10px;margin-bottom:9px}.api-response-card>header b{color:var(--primary)}.api-response-card>header span{color:var(--muted)}.api-warnings{padding:12px;border-radius:8px;background:color-mix(in srgb,var(--warning) 10%,var(--surface))}.api-warnings p{color:var(--warning)}.api-detail-empty{margin:auto;text-align:center;color:var(--muted)}.api-detail-empty b{color:var(--text);font-size:16px}.api-source-editor>p{padding:0 18px;color:var(--muted)}
.api-detail-empty{padding:20px}
.api-source-actions{display:flex!important;align-items:center;gap:6px}
.api-tree-toggle{min-width:0;flex:1;display:flex;align-items:center;gap:7px;border:0;background:transparent;color:inherit;text-align:left;padding:0}.api-tree-toggle b{min-width:0;flex:1}.api-tag-export-trigger{flex:0 0 auto;border:1px solid var(--line);border-radius:5px;background:var(--surface-2);color:var(--muted);font-size:9px;padding:4px 6px}.api-tag-export-trigger:hover{border-color:var(--primary);color:var(--primary)}
.api-path-copy{display:flex;align-items:center;gap:7px;border:0;background:transparent;padding:0;color:var(--muted);cursor:pointer}.api-path-copy:hover code,.api-path-copy:hover span{color:var(--primary)}.api-path-copy span{font-size:10px}
.api-detail-loading{margin:auto;display:grid;justify-items:center;gap:10px;padding:28px;text-align:center;color:var(--muted)}.api-detail-loading i{width:34px;height:34px;border:3px solid var(--line);border-top-color:var(--primary);border-radius:50%;animation:api-detail-spin .8s linear infinite}.api-detail-loading b{color:var(--text);font-size:15px}.api-detail-loading span{font-size:12px}@keyframes api-detail-spin{to{transform:rotate(360deg)}}
.api-test-result{border:1px solid var(--line);border-radius:9px;padding:14px;background:var(--surface-2)}.api-test-result.success{border-color:color-mix(in srgb,var(--success) 45%,var(--line))}.api-test-result.error{border-color:color-mix(in srgb,var(--danger) 45%,var(--line))}.api-test-result>header{display:flex;justify-content:space-between;gap:12px;align-items:flex-start;margin-bottom:12px}.api-test-result>header h3,.api-test-result>header p{margin:0}.api-test-result>header span{max-width:55%;color:var(--muted);font-size:11px;word-break:break-all}.api-test-result-grid{display:grid;grid-template-columns:minmax(0,.8fr) minmax(0,1.2fr);gap:10px}.api-test-result-grid article{min-width:0;border:1px solid var(--line);border-radius:7px;padding:10px;background:var(--surface)}.api-test-result-grid article>b{display:block;margin-bottom:7px}.api-test-result-grid small{display:block;color:var(--muted);margin-bottom:7px}.api-test-result-grid pre{max-height:300px;overflow:auto;margin:0;padding:10px;border-radius:6px;background:var(--surface-2);white-space:pre-wrap;word-break:break-word;font-size:12px;line-height:1.6}.api-test-result-grid p{color:var(--warning);font-size:11px}
.api-tag-export-editor>p{padding:0 18px;color:var(--muted);line-height:1.6}.api-project-settings-editor{width:min(760px,calc(100vw - 40px));height:100%;max-height:none;overflow:hidden;padding:0}.api-project-settings-editor>header{flex:0 0 auto;margin:0;padding:18px;border-bottom:1px solid var(--line)}.api-project-settings-editor>footer{flex:0 0 auto;margin:0;padding:14px 18px;border-top:1px solid var(--line);background:var(--surface)}.api-project-settings-scroll{flex:1;min-height:0;overflow-y:auto;padding:16px 18px;display:grid;align-content:start;gap:16px}.api-project-settings-scroll>section{display:grid;gap:12px;padding:15px;border:1px solid var(--line);border-radius:9px;background:var(--surface-2)}.api-project-settings-scroll h3{margin:0;font-size:15px}.api-project-settings-scroll section>label{padding:0}.api-project-settings-scroll section>p{margin:0;color:var(--muted);font-size:11px;line-height:1.6}.api-credential-status{display:flex;align-items:center;justify-content:space-between;gap:12px;padding:9px 11px;border-radius:7px;background:var(--surface);color:var(--muted);font-size:12px}.api-clear-token{justify-self:start}
.api-test-preview-editor{width:min(720px,calc(100vw - 40px));max-height:calc(100vh - 56px);overflow:hidden}.api-test-preview-scroll{min-height:0;overflow-y:auto;padding:16px 18px;display:grid;gap:15px}.api-test-preview-scroll>label{padding:0}.api-test-preview-scroll textarea{width:100%;min-height:170px;resize:vertical;border:1px solid var(--line);border-radius:7px;background:var(--surface-2);color:var(--text);padding:10px;font:12px/1.6 Consolas,monospace}.api-preview-warning{display:grid;gap:5px;padding:11px;border:1px solid color-mix(in srgb,var(--warning) 45%,var(--line));border-radius:8px;background:color-mix(in srgb,var(--warning) 8%,var(--surface));color:var(--warning)}.api-preview-warning.danger{border-color:color-mix(in srgb,var(--danger) 45%,var(--line));background:color-mix(in srgb,var(--danger) 8%,var(--surface));color:var(--danger)}.api-preview-warning span{font-size:12px;line-height:1.6}.api-test-preview-scroll section{display:grid;gap:8px}.api-preview-headers{display:grid;border:1px solid var(--line);border-radius:7px;overflow:hidden}.api-preview-headers span{display:grid;grid-template-columns:minmax(130px,.5fr) minmax(0,1fr);gap:10px;padding:8px 10px;border-bottom:1px solid var(--line);font-size:12px}.api-preview-headers span:last-child{border-bottom:0}.api-preview-headers em{font-style:normal;color:var(--muted);word-break:break-all}.api-preview-headers small{padding:10px;color:var(--muted)}.api-test-preview-scroll details{border:1px solid var(--line);border-radius:7px;padding:10px}.api-test-preview-scroll details pre{max-height:220px;overflow:auto;white-space:pre-wrap;word-break:break-word;font-size:12px}.api-danger-confirm,.api-template-check{display:flex!important;align-items:flex-start!important;grid-template-columns:none!important;gap:9px!important}.api-danger-confirm{padding:11px!important;border:1px solid color-mix(in srgb,var(--danger) 40%,var(--line));border-radius:7px;color:var(--danger)!important}.api-danger-confirm input,.api-template-check input{width:auto!important;flex:0 0 auto;margin-top:2px}.api-code-template-editor{width:min(560px,calc(100vw - 40px))}.api-tag-export-editor input[readonly]{font-family:Consolas,monospace;color:var(--primary)}
.api-preview-error{padding:9px 11px;border-radius:7px;background:color-mix(in srgb,var(--danger) 10%,var(--surface));color:var(--danger);font-size:12px}
@media(max-width:1500px){.api-docs-layout{grid-template-columns:200px 320px minmax(0,1fr)}.api-docs-layout.source-collapsed{grid-template-columns:0 320px minmax(0,1fr)}}
</style>
