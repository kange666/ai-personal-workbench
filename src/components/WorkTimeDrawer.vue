<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { deleteWorkSession, getWorkSummary, isTauriRuntime, listWorkSessions, saveWorkSession, type WorkSession, type WorkSummary } from "../services/backend";
import { APP_BRAND } from "../utils/brand";

const props = defineProps<{ open: boolean; startDate: string; endDate: string; title?: string }>();
const emit = defineEmits<{ close: []; changed: [] }>();
const emptySummary = (): WorkSummary => ({ startDate: props.startDate, endDate: props.endDate, totalMinutes: 0, estimatedMinutes: 0, manualMinutes: 0, hasManualCorrections: false, byProject: [], byType: [], daily: [] });
const sessions = ref<WorkSession[]>([]);
const summary = ref<WorkSummary>(emptySummary());
const loading = ref(false);
const error = ref("");
const editing = ref(false);
const form = ref<WorkSession>(blankSession());
const workTypes = ["功能开发", "测试验证", "问题修复", "调研", "部署", "方案与文档"];
const projects = computed(() => [...new Set(sessions.value.map(item => item.project).filter(Boolean))]);

function today() { return new Date().toLocaleDateString("sv-SE"); }
function blankSession(date = props.startDate || today()): WorkSession {
  return { id: "", date, startTime: "09:00", endTime: "10:00", durationMinutes: 60, project: APP_BRAND.name, workType: "功能开发", source: "manual", note: "", createdAt: "", updatedAt: "" };
}
function formatMinutes(value: number) {
  const hours = Math.floor(value / 60); const minutes = value % 60;
  return `${hours ? `${hours}小时` : ""}${minutes || !hours ? `${minutes}分钟` : ""}`;
}
function sourceLabel(value: WorkSession["source"]) { return value === "manual" ? "手工记录" : "估算工时"; }
function syncDuration() {
  const [sh, sm] = form.value.startTime.split(":").map(Number); const [eh, em] = form.value.endTime.split(":").map(Number);
  const value = eh * 60 + em - sh * 60 - sm; if (value > 0) form.value.durationMinutes = value;
}
async function load() {
  if (!props.open || !props.startDate || !props.endDate || !isTauriRuntime()) return;
  loading.value = true; error.value = "";
  try { summary.value = await getWorkSummary(props.startDate, props.endDate, true); sessions.value = await listWorkSessions(props.startDate, props.endDate, false); }
  catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}
function addManual(date = props.startDate) { form.value = blankSession(date); editing.value = true; }
function correct(item: WorkSession) { form.value = { ...item, id: item.source === "estimated" ? "" : item.id, source: "manual", note: item.source === "estimated" ? `修正估算：${item.note}` : item.note }; editing.value = true; }
async function save() {
  loading.value = true; error.value = "";
  try { await saveWorkSession(form.value); editing.value = false; await load(); emit("changed"); }
  catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}
async function remove(item: WorkSession) { if (item.source !== "manual") return; loading.value = true; try { await deleteWorkSession(item.id); await load(); emit("changed"); } finally { loading.value = false; } }

watch(() => [props.open, props.startDate, props.endDate], () => { if (props.open) { editing.value = false; void load(); } }, { immediate: true });
</script>

<template>
  <div v-if="open" class="activity-backdrop" @click.self="emit('close')"><aside class="activity-drawer panel worktime-drawer"><header><div><h2>{{ title || `${startDate} 工时明细` }}</h2><p>自动结果仅为估算，可手工修正；重叠区间以手工记录为准</p></div><button class="icon-button" @click="emit('close')">×</button></header>
    <div v-if="error" class="scan-message error">{{ error }}</div>
    <div class="worktime-summary"><div><small>有效工时</small><b>{{ formatMinutes(summary.totalMinutes) }}</b><span>{{ summary.hasManualCorrections ? '含手工修正' : '全部为估算' }}</span></div><div><small>原始估算</small><b>{{ formatMinutes(summary.estimatedMinutes) }}</b><span>保留用于对比</span></div><div><small>手工记录</small><b>{{ formatMinutes(summary.manualMinutes) }}</b><span>重叠时优先</span></div></div>
    <div class="worktime-actions"><button class="button primary small" @click="addManual()">＋ 补录工时</button><button class="button secondary small" :disabled="loading" @click="load">↻ 重新估算</button></div>
    <section class="worktime-breakdowns"><div><h3>按项目</h3><p v-for="item in summary.byProject" :key="item.name"><span>{{ item.name }}</span><b>{{ formatMinutes(item.minutes) }}</b></p></div><div><h3>按类型</h3><p v-for="item in summary.byType" :key="item.name"><span>{{ item.name }}</span><b>{{ formatMinutes(item.minutes) }}</b></p></div></section>
    <section class="worktime-list"><h3>工作时间段</h3><article v-for="item in sessions" :key="item.id" :class="item.source"><div><b>{{ item.startTime }}—{{ item.endTime }}</b><span>{{ item.project }} · {{ item.workType }}</span><small>{{ item.note || '无备注' }}</small></div><aside><em>{{ formatMinutes(item.durationMinutes) }}</em><i>{{ sourceLabel(item.source) }}</i><button class="text-button" @click="correct(item)">{{ item.source === 'manual' ? '编辑' : '手工修正' }}</button><button v-if="item.source === 'manual'" class="text-button danger-text" @click="remove(item)">删除</button></aside></article><p v-if="!sessions.length && !loading" class="panel-empty">当前周期没有可估算的本地活动，可手工补录。</p></section>
    <div v-if="editing" class="worktime-editor"><header><h3>{{ form.id ? '编辑手工工时' : '补录或修正工时' }}</h3><button class="icon-button" @click="editing = false">×</button></header><div class="form-grid"><label>日期<input v-model="form.date" type="date"></label><label>实际工时（分钟）<input v-model.number="form.durationMinutes" type="number" min="1" max="1440"></label><label>开始时间<input v-model="form.startTime" type="time" @change="syncDuration"></label><label>结束时间<input v-model="form.endTime" type="time" @change="syncDuration"></label></div><label>所属项目<input v-model="form.project" list="worktime-projects"><datalist id="worktime-projects"><option v-for="project in projects" :key="project" :value="project" /></datalist></label><label>工作类型<select v-model="form.workType"><option v-for="type in workTypes" :key="type">{{ type }}</option></select></label><label>备注<textarea v-model="form.note" rows="3" placeholder="例如：开发用户管理详情页面"></textarea></label><footer><button class="button secondary" @click="editing = false">取消</button><button class="button primary" :disabled="loading" @click="save">保存手工记录</button></footer></div>
  </aside></div>
</template>
