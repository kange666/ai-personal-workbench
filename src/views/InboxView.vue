<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { createTaskFromInbox, isTauriRuntime, listInboxItems, updateInboxStatus, type InboxItem, type InboxWorkflowStatus } from "../services/backend";
import { compactDetailTitle } from "../utils/detailTitle";
import { cleanTestOutput } from "../utils/testFailureDiagnosis";
import { useWorkbenchStore } from "../stores/workbench";

const route=useRoute();
const router=useRouter();
const store=useWorkbenchStore();
const items=ref<InboxItem[]>([]);
const activeFilter=ref<"open"|InboxWorkflowStatus>("open");
const projectFilter=ref("全部项目");
const sourceFilter=ref("全部来源");
const query=ref("");
const selected=ref<InboxItem|null>(null);
const loading=ref(false);
const message=ref("");
const error=ref("");

const sourceLabels:Record<string,string>={codex:"Codex",tapd:"TAPD",tapd_job:"TAPD 自动处理",task_suggestion:"任务建议",quick_capture:"快速任务",test:"测试",repository:"项目资产"};
const statusLabels:Record<string,string>={needs_decision:"待决策",in_progress:"处理中",done:"已完成",archived:"已归档"};
const taskStatusLabels:Record<string,string>={draft:"待安排",todo:"待办",doing:"进行中",done:"已完成",cancelled:"已取消"};
const projects=computed(()=>["全部项目",...Array.from(new Set(items.value.map(item=>item.project))).sort((a,b)=>a.localeCompare(b,"zh-CN"))]);
const sources=computed(()=>["全部来源",...Array.from(new Set(items.value.map(item=>item.sourceType))).map(value=>({value,label:sourceLabels[value]||value}))]);
const counts=computed(()=>({
  open:items.value.filter(item=>["needs_decision","in_progress"].includes(item.workflowStatus)).length,
  needs_decision:items.value.filter(item=>item.workflowStatus==="needs_decision").length,
  in_progress:items.value.filter(item=>item.workflowStatus==="in_progress").length,
  done:items.value.filter(item=>item.workflowStatus==="done").length,
  archived:items.value.filter(item=>item.workflowStatus==="archived").length,
}));
const filtered=computed(()=>{
  const keyword=query.value.trim().toLowerCase();
  return items.value.filter(item=>{
    const statusMatch=activeFilter.value==="open" ? ["needs_decision","in_progress"].includes(item.workflowStatus) : item.workflowStatus===activeFilter.value;
    const projectMatch=projectFilter.value==="全部项目" || item.project===projectFilter.value;
    const sourceMatch=sourceFilter.value==="全部来源" || item.sourceType===sourceFilter.value;
    const keywordMatch=!keyword || [item.title,item.summary,item.project,sourceLabels[item.sourceType]].some(value=>(value||"").toLowerCase().includes(keyword));
    return statusMatch&&projectMatch&&sourceMatch&&keywordMatch;
  });
});

function formatTime(value:string){const date=new Date(value);return Number.isNaN(date.getTime())?value:new Intl.DateTimeFormat("zh-CN",{month:"2-digit",day:"2-digit",hour:"2-digit",minute:"2-digit"}).format(date);}
function priorityLabel(value:string){return value==="high"?"高优先级":value==="low"?"低优先级":"普通";}
function displayTitle(value:string){return compactDetailTitle(cleanTestOutput(value));}
function displaySummary(item:InboxItem){return cleanTestOutput(item.summary||item.detail||"暂无补充说明").replace(/\s+/g," ");}

async function load(){
  if(!isTauriRuntime())return;
  loading.value=true;error.value="";
  try{
    items.value=(await listInboxItems(undefined,300)).filter(item=>item.sourceType!=="video");
    const target=String(route.query.item||selected.value?.id||"");
    if(target) selected.value=items.value.find(item=>item.id===target)||null;
  }catch(cause){error.value=String(cause);}finally{loading.value=false;}
}

async function changeStatus(item:InboxItem,status:InboxWorkflowStatus){
  if(loading.value)return;
  loading.value=true;error.value="";message.value="";
  try{
    await updateInboxStatus(item.id,status);
    item.workflowStatus=status;
    if(item.linkedTaskId&&status!=="archived")item.linkedTaskStatus=status==="done"?"done":status==="in_progress"?"doing":"todo";
    item.updatedAt=new Date().toISOString();
    message.value=`“${compactDetailTitle(item.title)}”已标记为${statusLabels[status]}。`;
    if(selected.value?.id===item.id)selected.value={...item};
    if(item.linkedTaskId)await syncTaskList("事项状态已保存",item);
  }catch(cause){error.value=String(cause);}finally{loading.value=false;}
}

async function createTask(item:InboxItem){
  if(loading.value)return;
  if(item.linkedTaskId){await openTask(item);return;}
  loading.value=true;error.value="";message.value="";
  try{
    const taskId=await createTaskFromInbox(item.id);
    item.workflowStatus="in_progress";
    item.linkedTaskId=taskId;
    item.linkedTaskStatus=null;
    message.value="已转为今日任务，可以在工作日历继续安排。";
    await showLinkedTask(item,"任务已安排成功");
  }catch(cause){error.value=String(cause);}finally{loading.value=false;}
}

async function openTask(item:InboxItem){
  if(loading.value||!item.linkedTaskId)return;
  loading.value=true;error.value="";message.value="";
  try{await showLinkedTask(item,"任务已关联");}finally{loading.value=false;}
}

// 后端写入已成功，后续读取失败不能回退关联关系或误报保存失败。
async function syncTaskList(savedMessage:string,item:InboxItem){
  try{
    // 正在进行的查询可能早于本次后端写入；先等它结束，再发起新查询。
    if(store.hydrating)await store.hydrate().catch(()=>undefined);
    await store.hydrate();
    item.linkedTaskStatus=store.tasks.find(task=>task.id===item.linkedTaskId)?.status||null;
    if(selected.value?.id===item.id)selected.value={...item};
    return true;
  }
  catch(cause){error.value=`${savedMessage}，但任务列表同步失败；可点击“查看任务”重试，不会重复安排：${String(cause)}`;return false;}
}

async function showLinkedTask(item:InboxItem,savedMessage:string){
  if(!await syncTaskList(savedMessage,item))return;
  if(!store.tasks.some(task=>task.id===item.linkedTaskId)){error.value="关联任务未读取到，请同步来源后重试查看。";return;}
  try{await router.push(`/calendar?tab=tasks&task=${encodeURIComponent(item.linkedTaskId!)}`);}
  catch(cause){error.value=`${savedMessage}，但打开任务页面失败，可重试查看：${String(cause)}`;}
}

async function openSource(item:InboxItem){
  if(!item.route)return;
  selected.value=null;
  if(/^https?:/i.test(item.route))window.open(item.route,"_blank","noopener,noreferrer");else await router.push(item.route);
}

onMounted(load);
</script>

<template>
  <div class="view inbox-view">
    <header class="page-header"><div><h1>待处理收件箱</h1></div><div><button class="button secondary" :disabled="loading" @click="load">{{ loading?"同步中…":"↻ 同步来源" }}</button><RouterLink class="button primary link-button" to="/calendar?tab=tasks">打开工作日历</RouterLink></div></header>
    <div v-if="message||error" class="inbox-message" :class="{error:Boolean(error)}">{{ error||message }}</div>
    <section class="inbox-metrics">
      <article><small>需要行动</small><b>{{ counts.open }}</b></article>
      <article><small>待决策</small><b>{{ counts.needs_decision }}</b></article>
      <article><small>处理中</small><b>{{ counts.in_progress }}</b></article>
      <article><small>已完成</small><b>{{ counts.done }}</b></article>
    </section>
    <section class="inbox-workspace panel">
      <nav class="inbox-tabs">
        <button v-for="tab in [{value:'open',label:'需要行动'},{value:'needs_decision',label:'待决策'},{value:'in_progress',label:'处理中'},{value:'done',label:'已完成'},{value:'archived',label:'已归档'}]" :key="tab.value" :class="{active:activeFilter===tab.value}" @click="activeFilter=tab.value as typeof activeFilter"><span>{{ tab.label }}</span><b>{{ counts[tab.value as keyof typeof counts] }}</b></button>
      </nav>
      <div class="inbox-toolbar"><select v-model="projectFilter"><option v-for="item in projects" :key="item">{{ item }}</option></select><select v-model="sourceFilter"><option value="全部来源">全部来源</option><option v-for="item in sources.slice(1)" :key="typeof item==='string'?item:item.value" :value="typeof item==='string'?item:item.value">{{ typeof item==='string'?item:item.label }}</option></select><label>⌕<input v-model="query" placeholder="搜索标题、项目或摘要"></label><span>{{ filtered.length }} 条</span></div>
      <div class="inbox-list">
        <article v-for="item in filtered" :key="item.id" :class="[`priority-${item.priority}`,`status-${item.workflowStatus}`]">
          <button class="inbox-main" @click="selected=item"><i></i><span><small>{{ sourceLabels[item.sourceType]||item.sourceType }} · {{ item.project }}</small><b>{{ displayTitle(item.title) }}</b><p>{{ displaySummary(item) }}</p></span></button>
          <div class="inbox-meta"><span :class="`priority-${item.priority}`">{{ priorityLabel(item.priority) }}</span><span>{{ statusLabels[item.workflowStatus] }}</span><small v-if="item.linkedTaskId">任务·{{ taskStatusLabels[item.linkedTaskStatus||'']||item.linkedTaskStatus||'待同步' }}</small><time>{{ formatTime(item.updatedAt) }}</time></div>
          <div class="inbox-actions">
            <button class="text-button inbox-action-detail" @click="selected=item">详情</button>
            <button v-if="item.linkedTaskId" class="button secondary small inbox-action-task" :disabled="loading" @click="openTask(item)">查看任务</button>
            <button v-else-if="['needs_decision','in_progress'].includes(item.workflowStatus)" class="button primary small inbox-action-task" :disabled="loading" @click="createTask(item)">安排任务</button>
            <button v-if="item.workflowStatus==='in_progress'" class="button primary small inbox-action-progress" :disabled="loading" @click="changeStatus(item,'done')">标记完成</button>
            <button v-else-if="item.workflowStatus==='done'||(item.workflowStatus==='needs_decision'&&item.linkedTaskId)" class="button secondary small inbox-action-progress" :disabled="loading" @click="changeStatus(item,'in_progress')">重新处理</button>
            <button v-else-if="item.workflowStatus==='archived'" class="button secondary small inbox-action-progress" :disabled="loading" @click="changeStatus(item,'needs_decision')">重新打开</button>
            <button v-if="item.workflowStatus!=='archived'" class="text-button muted inbox-action-archive" :disabled="loading" @click="changeStatus(item,'archived')">归档</button>
          </div>
        </article>
        <div v-if="!filtered.length" class="inbox-empty"><b>{{ loading?'正在同步本地来源':'当前筛选条件下没有事项' }}</b></div>
      </div>
    </section>
    <div v-if="selected" class="activity-backdrop" @click.self="selected=null"><aside class="activity-drawer panel inbox-detail"><header><div><small>{{ sourceLabels[selected.sourceType] }} · {{ selected.project }}</small><h2 :title="selected.title">{{ displayTitle(selected.title) }}</h2><p>{{ statusLabels[selected.workflowStatus] }} · {{ formatTime(selected.updatedAt) }}</p></div><button class="icon-button" @click="selected=null">×</button></header><section class="inbox-detail-tags"><span :class="`priority-${selected.priority}`">{{ priorityLabel(selected.priority) }}</span><span>{{ selected.sourceStatus||"来源状态未知" }}</span><span v-if="selected.linkedTaskId">关联任务：{{ taskStatusLabels[selected.linkedTaskStatus||'']||selected.linkedTaskStatus||'待同步' }}</span></section><section><h3>事项摘要</h3><p>{{ cleanTestOutput(selected.summary)||"暂无摘要" }}</p></section><section><h3>来源与处理说明</h3><p>{{ cleanTestOutput(selected.detail)||"暂无补充信息" }}</p><dl><div><dt>来源类型</dt><dd>{{ sourceLabels[selected.sourceType] }}</dd></div><div><dt>来源 ID</dt><dd>{{ selected.sourceId||"未关联" }}</dd></div><div><dt>规范项目</dt><dd>{{ selected.project }}</dd></div></dl></section><footer>
      <button v-if="selected.route" class="button secondary" @click="openSource(selected)">查看来源</button>
      <button v-if="selected.linkedTaskId" class="button secondary" :disabled="loading" @click="openTask(selected)">查看任务</button>
      <button v-else-if="['needs_decision','in_progress'].includes(selected.workflowStatus)" class="button primary" :disabled="loading" @click="createTask(selected)">安排任务</button>
      <button v-if="selected.workflowStatus==='in_progress'" class="button primary" :disabled="loading" @click="changeStatus(selected,'done')">标记完成</button>
      <button v-else-if="selected.workflowStatus==='done'||(selected.workflowStatus==='needs_decision'&&selected.linkedTaskId)" class="button primary" :disabled="loading" @click="changeStatus(selected,'in_progress')">重新处理</button>
      <button v-else-if="selected.workflowStatus==='archived'" class="button primary" :disabled="loading" @click="changeStatus(selected,'needs_decision')">重新打开</button>
    </footer></aside></div>
  </div>
</template>

<style scoped>
.inbox-actions>*{grid-row:1}.inbox-meta small{color:var(--muted);font-size:10px;white-space:nowrap}.inbox-detail-tags{flex-wrap:wrap}
.inbox-message{margin-bottom:12px;border:1px solid color-mix(in srgb,var(--success) 35%,var(--line));border-radius:8px;background:color-mix(in srgb,var(--success) 8%,var(--surface));color:var(--success);padding:11px 14px}.inbox-message.error{border-color:color-mix(in srgb,var(--danger) 40%,var(--line));background:color-mix(in srgb,var(--danger) 8%,var(--surface));color:var(--danger)}
.inbox-metrics{display:grid;grid-template-columns:repeat(4,1fr);gap:12px;margin-bottom:12px}.inbox-metrics article{min-height:94px;border:1px solid var(--line);border-radius:9px;background:var(--surface);padding:14px 16px}.inbox-metrics small,.inbox-metrics span{color:var(--muted)}.inbox-metrics b{display:block;margin:7px 0 4px;font-size:28px}.inbox-workspace{overflow:hidden}.inbox-tabs{height:54px;border-bottom:1px solid var(--line);display:flex;align-items:center;gap:6px;padding:0 12px}.inbox-tabs button{height:34px;border:1px solid transparent;border-radius:8px;background:transparent;color:var(--muted);padding:0 12px;display:flex;align-items:center;gap:7px}.inbox-tabs button b{min-width:20px;border-radius:999px;background:var(--surface-2);padding:3px 6px;font-size:9px}.inbox-tabs button.active{border-color:color-mix(in srgb,var(--primary) 35%,var(--line));background:var(--primary-soft);color:var(--primary);font-weight:800}.inbox-toolbar{min-height:58px;border-bottom:1px solid var(--line);display:grid;grid-template-columns:minmax(180px,.8fr) minmax(150px,.45fr) minmax(240px,1fr) auto;align-items:center;gap:10px;padding:10px 14px}.inbox-toolbar select,.inbox-toolbar label{width:100%;min-width:0;height:36px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);color:var(--text);padding:0 10px}.inbox-toolbar label{display:flex;align-items:center;gap:8px}.inbox-toolbar input{width:100%;min-width:0;flex:1;border:0;outline:0;background:transparent;color:var(--text)}.inbox-toolbar>span{min-width:58px;color:var(--muted);text-align:right;white-space:nowrap}.inbox-list{max-height:610px;overflow:auto}.inbox-list>article{min-height:112px;border-bottom:1px solid var(--line);display:grid;grid-template-columns:minmax(0,1fr) 112px 266px;align-items:center;gap:18px;padding:14px 16px}.inbox-list>article:hover{background:color-mix(in srgb,var(--primary) 3%,transparent)}.inbox-main{min-width:0;border:0;background:transparent;color:inherit;display:grid;grid-template-columns:8px minmax(0,1fr);gap:10px;text-align:left}.inbox-main>i{width:7px;height:7px;margin-top:20px;border-radius:50%;background:var(--muted)}.priority-high .inbox-main>i{background:var(--danger);box-shadow:0 0 0 3px color-mix(in srgb,var(--danger) 10%,transparent)}.priority-normal .inbox-main>i{background:var(--warning)}.inbox-main>span{min-width:0;display:grid;gap:5px}.inbox-main small{color:var(--primary)}.inbox-main b,.inbox-main p{white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.inbox-main p{margin:0;color:var(--muted)}.inbox-meta{display:grid;justify-items:start;align-content:center;gap:6px}.inbox-meta span{padding:4px 7px;border-radius:999px;background:var(--surface-2);color:var(--muted);font-size:9px;white-space:nowrap}.inbox-meta .priority-high,.inbox-detail-tags .priority-high{color:var(--danger);background:color-mix(in srgb,var(--danger) 10%,transparent)}.inbox-meta .priority-normal{color:var(--warning);background:color-mix(in srgb,var(--warning) 10%,transparent)}.inbox-meta time{color:var(--muted);font-size:9px;white-space:nowrap}.inbox-actions{width:266px;display:grid;grid-template-columns:40px 78px 78px 40px;align-items:center;justify-content:end;gap:6px}.inbox-actions>*{justify-self:stretch;white-space:nowrap}.inbox-actions .text-button{margin-left:0;text-align:center}.inbox-action-detail{grid-column:1}.inbox-action-progress{grid-column:2}.inbox-action-task{grid-column:3}.inbox-action-archive{grid-column:4}.inbox-actions .muted{color:var(--muted)}.inbox-empty{min-height:260px;display:grid;place-content:center;text-align:center;color:var(--muted)}.inbox-empty b{color:var(--text);font-size:15px}.inbox-empty p{margin:8px 0}.inbox-detail{width:640px}.inbox-detail>header>div{min-width:0}.inbox-detail h2{margin:5px 0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.inbox-detail-tags{display:flex!important;flex-direction:row!important;gap:8px!important;border:0!important;background:transparent!important}.inbox-detail-tags span{padding:6px 9px;border-radius:999px;background:var(--surface-2);color:var(--muted);font-size:10px}.inbox-detail>section{margin:16px;padding:15px;border:1px solid var(--line);border-radius:9px;background:var(--surface-2);display:grid;gap:10px}.inbox-detail h3{margin:0}.inbox-detail p{margin:0;color:var(--muted);line-height:1.7;white-space:pre-wrap}.inbox-detail dl{margin:4px 0 0}.inbox-detail dl div{display:grid;grid-template-columns:100px minmax(0,1fr);gap:12px;padding:9px 0;border-top:1px solid var(--line)}.inbox-detail dt{color:var(--muted)}.inbox-detail dd{margin:0;overflow-wrap:anywhere}.inbox-detail>footer{justify-content:flex-end}
@media(max-width:1280px){.inbox-list>article{grid-template-columns:minmax(0,1fr) 104px}.inbox-actions{grid-column:1/3;justify-self:end}.inbox-toolbar{grid-template-columns:minmax(160px,1fr) minmax(140px,.7fr) minmax(220px,1.2fr) auto}}
@media(max-width:1100px){.inbox-meta{display:none}.inbox-list>article{grid-template-columns:minmax(0,1fr)}.inbox-actions{grid-column:1;justify-self:end}.inbox-toolbar{grid-template-columns:1fr 1fr}.inbox-toolbar label{grid-column:1/2}.inbox-toolbar>span{grid-column:2;text-align:right}.inbox-metrics{grid-template-columns:1fr 1fr}}
</style>
