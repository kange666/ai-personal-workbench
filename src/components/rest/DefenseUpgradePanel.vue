<script setup lang="ts">
import { computed, ref } from 'vue';
import { defenseChoiceCatalog, defenseFeatures, type DefensePreference, type DefenseState } from '../../utils/rest/defense';
import { DEFENSE_FAMILIES } from '../../utils/rest/defenseContent';
import { defenseCardDetails } from '../../utils/rest/defenseCardDetails';
const props=defineProps<{state:DefenseState;paused:boolean}>();
const emit=defineEmits<{pick:[id:string,catalog:boolean];refresh:[];close:[];preference:[value:DefensePreference]}>();
const query=ref(''),family=ref<DefensePreference>('全部'),all=ref(false);
const searching=computed(()=>!!query.value.trim()||family.value!=='全部'||all.value);
const entries=computed(()=>{
  const catalog=defenseChoiceCatalog(props.state,query.value,family.value);
  return (searching.value?catalog:props.state.offers.map(id=>catalog.find(c=>c.id===id)).filter(c=>!!c))
    .map(card=>({...card,details:defenseCardDetails(props.state,card.id)}));
});
function preference(event:Event){emit('preference',(event.target as HTMLSelectElement).value as DefensePreference);}
</script>
<template>
  <div class="defense-choice-overlay">
    <h3>选择强化 · 剩余 {{ state.pendingUpgrades }} 次</h3>
    <p>战斗已暂停。每选一项消耗1次，剩余次数可连续选择；刷新免费且不限次数。</p>
    <div class="defense-choice-tools">
      <input v-model="query" class="input" type="search" aria-label="搜索全部强化" placeholder="搜索名称、效果，例如：火、吸血、冰霜" maxlength="100" :disabled="paused" />
      <select v-model="family" class="input" aria-label="筛选强化类别" :disabled="paused"><option v-for="item in DEFENSE_FAMILIES" :key="item">{{ item }}</option></select>
      <label><input v-model="all" type="checkbox" :disabled="paused" />全部技能</label>
    </div>
    <div class="game-actions">
      <label class="defense-preference">抽牌偏好 <select class="input" aria-label="抽牌偏好" :value="defenseFeatures(state).preference" :disabled="paused" @change="preference"><option v-for="item in DEFENSE_FAMILIES" :key="item">{{ item }}</option></select></label>
      <button class="button" :disabled="paused" @click="emit('refresh')">刷新三选一</button>
      <button class="button primary" :disabled="paused" @click="emit('close')">暂不选择，继续战斗</button>
    </div>
    <p v-if="searching" class="defense-catalog-hint">搜索整个技能库，可直接选择已满足条件的强化；未解锁的进化会显示配方。</p>
    <div class="defense-choices" :class="{'defense-catalog':searching}" aria-label="选择强化">
      <button v-for="entry in entries" :key="entry.id" class="defense-card" :class="{'defense-evolution-card':entry.id.startsWith('evo:')}" :disabled="paused||!!entry.requirements.length" @click="emit('pick',entry.id,searching)">
        <small>{{ entry.family }} · {{ entry.level }} → {{ entry.level+1 }} 级</small>
        <strong>{{ entry.name }}</strong><span>{{ entry.details.description }}</span>
        <small v-if="entry.details.next" class="defense-next-value">{{ entry.details.next }}</small>
        <small v-if="entry.level>=6&&!entry.id.startsWith('evo:')">精研：本级额外增加4个百分点全局伤害</small>
        <small v-if="entry.requirements.length" class="defense-required">需要：{{ entry.requirements.join('、') }}</small>
      </button>
    </div>
    <p v-if="!entries.length" role="status">没有匹配的技能，请调整搜索词或类别。</p>
    <p class="defense-card-context">{{ defenseCardDetails(state,'power').context }} · 精研：强化超过6级后，每级另有4个百分点全局增伤</p>
  </div>
</template>
<style scoped>
.defense-choice-overlay{position:absolute;inset:0;z-index:2;display:flex;flex-direction:column;align-items:center;gap:10px;padding:18px;overflow-y:auto;overscroll-behavior:contain;background:color-mix(in srgb,var(--surface) 96%,transparent);color:var(--text);text-align:center}
h3{margin:0;font-size:19px}p{margin:0;color:var(--muted);font-size:11px;line-height:1.65}.defense-choice-tools{display:flex;align-items:center;gap:8px;width:min(760px,100%);flex-wrap:wrap}.defense-choice-tools>input{flex:1;min-width:190px}.input{min-height:32px;padding:6px 8px;font-size:11px;border:1px solid var(--line);background:var(--surface-soft,var(--surface));color:var(--text);border-radius:7px}.defense-choice-tools label,.defense-preference{font-size:11px;display:flex;align-items:center;gap:5px}.game-actions{margin:0;gap:7px}.button{font-size:11px}.defense-choices{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:10px;width:min(820px,100%);padding-bottom:4px}.defense-card{display:flex;flex-direction:column;gap:8px;min-width:0;text-align:left;padding:14px 12px;color:var(--text);background:var(--surface);border:1px solid var(--primary);border-radius:11px;cursor:pointer;white-space:normal;overflow-wrap:anywhere;transition:border-color .15s}.defense-card:hover{border-color:var(--text)}.defense-card:disabled{opacity:.55;cursor:default}.defense-card strong{font-size:14px;color:var(--primary)}.defense-card small,.defense-card span{font-size:11px;line-height:1.7;color:var(--muted)}.defense-card .defense-next-value{color:var(--success)}.defense-card .defense-required{color:var(--warning)}.defense-evolution-card{border-color:var(--warning)}.defense-card-context,.defense-catalog-hint{font-size:10px}
@media(max-width:650px){.defense-choice-overlay{padding:12px}.defense-choices{grid-template-columns:1fr}.defense-choice-tools>input{min-width:130px}h3{font-size:16px}}
</style>
