<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { archiveQuickCapture, deleteQuickCapture, isTauriRuntime, listQuickCaptures, saveQuickCapture, type QuickCapture } from "../services/backend";
import { confirmAction } from "../utils/confirm";

const props=defineProps<{ open:boolean }>();
const emit=defineEmits<{ close:[]; saved:[QuickCapture] }>();
const kind=ref<QuickCapture["kind"]>("note");
const content=ref("");
const sourceUrl=ref("");
const editingId=ref<string | null>(null);
let draft:Pick<QuickCapture,"kind"|"content"|"sourceUrl"> | null=null;
const items=ref<QuickCapture[]>([]);
const loading=ref(false);
const error=ref("");
const message=ref("");

const kindOptions:Array<{id:QuickCapture["kind"];label:string}>=[
  {id:"note",label:"笔记"},
  {id:"idea",label:"灵感"},
  {id:"url",label:"网址"},
  {id:"task",label:"任务"},
];
function kindLabel(value:QuickCapture["kind"]){return kindOptions.find(item=>item.id===value)?.label||"记录";}

function edit(item:QuickCapture) {
  if (loading.value || editingId.value===item.id) return;
  if (!editingId.value) draft={kind:kind.value,content:content.value,sourceUrl:sourceUrl.value};
  editingId.value=item.id;
  kind.value=item.kind; content.value=item.content; sourceUrl.value=item.sourceUrl;
  error.value=""; message.value="";
  void nextTick(()=>document.querySelector<HTMLTextAreaElement>(".quick-capture-editor textarea")?.focus());
}
function cancelEdit() {
  editingId.value=null;
  kind.value=draft?.kind ?? "note"; content.value=draft?.content ?? ""; sourceUrl.value=draft?.sourceUrl ?? "";
  draft=null; error.value=""; message.value="";
}

async function load() { if (isTauriRuntime()) items.value=await listQuickCaptures(); }
watch(()=>props.open,value=>{ if (value) { error.value=""; message.value=""; void load(); window.setTimeout(()=>document.querySelector<HTMLTextAreaElement>(".quick-capture-editor textarea")?.focus(),50); } });
watch(()=>props.open,value=>{ if (!value && editingId.value) cancelEdit(); });
async function save() {
  if (loading.value || !content.value.trim()) return;
  loading.value=true; error.value=""; message.value="";
  try {
    const id=editingId.value;
    const selectedKind=kind.value;
    const item=await saveQuickCapture({...(id ? {id} : {}),kind:selectedKind,content:content.value,sourceUrl:sourceUrl.value});
    if (id) {
      items.value=items.value.map(value=>value.id===id ? item : value);
      cancelEdit();
      message.value="修改已保存。";
    } else {
      if(item.status==="inbox")items.value.unshift(item);
      content.value=""; sourceUrl.value="";
      message.value=selectedKind==="task"?"任务已加入待处理收件箱。":"";
    }
    emit("saved",item);
  }
  catch(cause) { error.value=String(cause); }
  finally { loading.value=false; }
}
async function archive(item:QuickCapture) {
  if (loading.value) return;
  loading.value=true; error.value="";
  try {
    await archiveQuickCapture(item.id);
    items.value=items.value.filter(value=>value.id!==item.id);
    if (editingId.value===item.id) cancelEdit();
  }
  catch(cause) { error.value=String(cause); }
  finally { loading.value=false; }
}
async function remove(item:QuickCapture) {
  if (loading.value) return;
  if (!await confirmAction({ title:"删除快速记录", message:`确定删除这条${kindLabel(item.kind)}吗？删除后不能恢复。`, confirmText:"删除", tone:"danger" })) return;
  loading.value=true; error.value="";
  try { await deleteQuickCapture(item.id); items.value=items.value.filter(value=>value.id!==item.id); if (editingId.value===item.id) cancelEdit(); }
  catch(cause) { error.value=String(cause); }
  finally { loading.value=false; }
}
function time(value:string) { return new Intl.DateTimeFormat("zh-CN",{month:"numeric",day:"numeric",hour:"2-digit",minute:"2-digit"}).format(new Date(value)); }
function close() { if (!loading.value) emit("close"); }
</script>

<template>
  <div v-if="open" class="modal-backdrop quick-capture-backdrop" @click.self="close">
    <section class="panel quick-capture-dialog">
      <header><div><h2>快速记录</h2><p>{{ editingId ? '修改已有记录，保存后立即更新' : '任务将进入待处理收件箱' }}</p></div><button class="icon-button" :disabled="loading" @click="close">×</button></header>
      <div class="quick-capture-body">
        <div class="quick-capture-editor">
          <nav><button v-for="item in kindOptions" :key="item.id" :class="{active:kind===item.id}" :disabled="loading || editingId !== null" @click="kind=item.id">{{ item.label }}</button></nav>
          <textarea v-model="content" rows="8" :disabled="loading" :placeholder="kind==='note'?'随手记录一段信息…':kind==='idea'?'记录一个以后值得尝试的想法…':kind==='url'?'写一句为什么保存这个网址…':'记录需要处理的事项，保存后进入待处理收件箱…'" @keydown.ctrl.enter.prevent="save"></textarea>
          <input v-if="kind==='url'" v-model="sourceUrl" :disabled="loading" type="url" placeholder="https://…" @keydown.enter.prevent="save">
          <p v-if="error" class="form-error">{{ error }}</p>
          <p v-if="message" class="quick-capture-message">{{ message }}</p>
          <footer><small>{{ editingId ? '编辑中 · Ctrl + Enter 保存' : 'Ctrl + Enter 保存' }}</small><div class="quick-capture-editor-actions"><button v-if="editingId" class="button secondary" :disabled="loading" @click="cancelEdit">取消</button><button class="button primary" title="全局快捷键 Ctrl + Shift + Space" :disabled="loading || !content.trim()" @click="save">{{ loading?'保存中…':editingId?'保存修改':kind==='task'?'加入待处理':'保存记录' }}</button></div></footer>
        </div>
        <aside><h3>待整理记录 <span>{{ items.length }}</span></h3><div class="quick-capture-list"><article v-for="item in items" :key="item.id" :class="{'is-editing':editingId===item.id}"><div class="quick-capture-copy"><small>{{ kindLabel(item.kind) }} · {{ time(item.createdAt) }}</small><p>{{ item.content }}</p><a v-if="item.sourceUrl" :href="item.sourceUrl" target="_blank" rel="noreferrer">{{ item.sourceUrl }}</a></div><div class="quick-capture-actions"><button class="text-button" :disabled="loading" @click="edit(item)">{{ editingId===item.id ? '编辑中' : '编辑' }}</button><button class="text-button" :disabled="loading" @click="archive(item)">归档</button><button class="text-button danger-text" :disabled="loading" @click="remove(item)">删除</button></div></article><p v-if="!items.length" class="panel-empty">暂无待整理记录。</p></div></aside>
      </div>
    </section>
  </div>
</template>

<style scoped>
.quick-capture-message{margin:0;color:var(--success);font-weight:700}
.quick-capture-editor-actions{display:flex;gap:8px;flex-wrap:wrap}
.quick-capture-editor footer{flex-wrap:wrap}
.quick-capture-list article.is-editing{background:var(--primary-soft)}
.quick-capture-copy p{overflow-wrap:anywhere}
.quick-capture-actions{flex-wrap:wrap;justify-content:flex-end;max-width:9em}
</style>
