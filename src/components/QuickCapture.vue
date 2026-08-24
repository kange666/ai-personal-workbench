<script setup lang="ts">
import { ref, watch } from "vue";
import { archiveQuickCapture, deleteQuickCapture, isTauriRuntime, listQuickCaptures, saveQuickCapture, type QuickCapture } from "../services/backend";

const props=defineProps<{ open:boolean }>();
const emit=defineEmits<{ close:[]; saved:[QuickCapture] }>();
const kind=ref<QuickCapture["kind"]>("note");
const content=ref("");
const sourceUrl=ref("");
const items=ref<QuickCapture[]>([]);
const loading=ref(false);
const error=ref("");

async function load() { if (isTauriRuntime()) items.value=await listQuickCaptures(); }
watch(()=>props.open,value=>{ if (value) { error.value=""; void load(); window.setTimeout(()=>document.querySelector<HTMLTextAreaElement>(".quick-capture-editor textarea")?.focus(),50); } });
async function save() {
  loading.value=true; error.value="";
  try { const item=await saveQuickCapture({kind:kind.value,content:content.value,sourceUrl:sourceUrl.value}); items.value.unshift(item); content.value=""; sourceUrl.value=""; emit("saved",item); }
  catch(cause) { error.value=String(cause); }
  finally { loading.value=false; }
}
async function archive(item:QuickCapture) { await archiveQuickCapture(item.id); items.value=items.value.filter(value=>value.id!==item.id); }
async function remove(item:QuickCapture) {
  if (!window.confirm(`确定删除这条${item.kind==='note'?'笔记':item.kind==='idea'?'灵感':'网址'}吗？删除后不能恢复。`)) return;
  loading.value=true; error.value="";
  try { await deleteQuickCapture(item.id); items.value=items.value.filter(value=>value.id!==item.id); }
  catch(cause) { error.value=String(cause); }
  finally { loading.value=false; }
}
function time(value:string) { return new Intl.DateTimeFormat("zh-CN",{month:"numeric",day:"numeric",hour:"2-digit",minute:"2-digit"}).format(new Date(value)); }
</script>

<template>
  <div v-if="open" class="modal-backdrop quick-capture-backdrop" @click.self="emit('close')">
    <section class="panel quick-capture-dialog">
      <header><div><h2>快速记录</h2><p>记下笔记、灵感或网址，不会自动变成每日任务。</p></div><button class="icon-button" @click="emit('close')">×</button></header>
      <div class="quick-capture-body">
        <div class="quick-capture-editor">
          <nav><button v-for="item in [{id:'note',label:'笔记'},{id:'idea',label:'灵感'},{id:'url',label:'网址'}]" :key="item.id" :class="{active:kind===item.id}" @click="kind=item.id as QuickCapture['kind']">{{ item.label }}</button></nav>
          <textarea v-model="content" rows="8" :placeholder="kind==='note'?'随手记录一段信息…':kind==='idea'?'记录一个以后值得尝试的想法…':'写一句为什么保存这个网址…'" @keydown.ctrl.enter="save"></textarea>
          <input v-if="kind==='url'" v-model="sourceUrl" type="url" placeholder="https://…" @keydown.enter="save">
          <p v-if="error" class="form-error">{{ error }}</p>
          <footer><small>全局快捷键 Ctrl + Shift + Space · Ctrl + Enter 保存</small><button class="button primary" :disabled="loading || !content.trim()" @click="save">{{ loading?'保存中…':'保存记录' }}</button></footer>
        </div>
        <aside><h3>待整理记录 <span>{{ items.length }}</span></h3><div class="quick-capture-list"><article v-for="item in items" :key="item.id"><div class="quick-capture-copy"><small>{{ item.kind==='note'?'笔记':item.kind==='idea'?'灵感':'网址' }} · {{ time(item.createdAt) }}</small><p>{{ item.content }}</p><a v-if="item.sourceUrl" :href="item.sourceUrl" target="_blank" rel="noreferrer">{{ item.sourceUrl }}</a></div><div class="quick-capture-actions"><button class="text-button" :disabled="loading" @click="archive(item)">归档</button><button class="text-button danger-text" :disabled="loading" @click="remove(item)">删除</button></div></article><p v-if="!items.length" class="panel-empty">暂无待整理记录。</p></div></aside>
      </div>
    </section>
  </div>
</template>
