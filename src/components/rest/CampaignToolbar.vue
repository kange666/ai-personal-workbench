<script setup lang="ts">
import { computed } from 'vue';
const props = defineProps<{ mode: string; level: number; levels: readonly { name: string; chapter: string }[]; stars: number[]; unlocked: number; modes?: { id: string; label?: string; name?: string }[]; disabled?: boolean }>();
const choices = computed<{id:string;label?:string;name?:string}[]>(()=>props.modes || [{id:'campaign',label:'章节闯关'},{id:'endless',label:'无尽挑战'}]);
defineEmits<{ mode: [value: string]; select: [level: number] }>();
</script>
<template>
  <div class="campaign-toolbar">
    <label>玩法 <select :value="mode" :disabled="disabled" @change="$emit('mode', ($event.target as HTMLSelectElement).value)"><option v-for="choice in choices" :key="choice.id" :value="choice.id">{{ choice.label || choice.name }}</option></select></label>
    <label v-if="!['endless','legacy'].includes(mode)">关卡 <select :value="level" :disabled="disabled" @change="$emit('select', Number(($event.target as HTMLSelectElement).value))"><option v-for="(entry,i) in levels" :key="i" :value="i" :disabled="i > unlocked">{{ String(i+1).padStart(2,'0') }} · {{ entry.chapter }} · {{ entry.name }} {{ '★'.repeat(stars[i] || 0) }}{{ i > unlocked ? '（待解锁）' : '' }}</option></select></label>
    <span v-if="!['endless','legacy'].includes(mode)" class="campaign-stars">{{ stars.reduce((a,b)=>a+b,0) }}/{{ levels.length*3 }} ★</span>
  </div>
</template>
<style scoped>
.campaign-toolbar{display:flex;gap:9px;align-items:center;flex-wrap:wrap;margin-bottom:12px;font-size:11px;color:var(--muted)}
.campaign-toolbar label{display:flex;align-items:center;gap:5px;min-width:0}.campaign-toolbar label:nth-child(2){flex:1}
.campaign-toolbar select{max-width:100%;min-width:0;min-height:30px;border:1px solid var(--line);border-radius:7px;background:var(--surface-2);padding:4px 7px;color:var(--text);font:inherit}.campaign-stars{color:var(--warning);font-variant-numeric:tabular-nums}
</style>
