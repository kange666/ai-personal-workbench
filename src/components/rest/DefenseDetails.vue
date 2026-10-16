<script setup lang="ts">
import { computed, ref, toRaw } from 'vue';
import { defenseFeatures, defenseRunReview, type DefenseState } from '../../utils/rest/defense';
import { DEFENSE_RULES, defenseNumber, defensePropertyGroups } from '../../utils/rest/defenseCardDetails';
const props=defineProps<{state:DefenseState;revision?:number}>();
// 折叠时不计算详情，展开后随界面时钟更新，避免几十条属性跟随每帧重排。
const open=ref(false),snapshot=computed(()=>{void props.revision;return{groups:defensePropertyGroups(toRaw(props.state)),review:defenseRunReview(toRaw(props.state))};});
const groups=computed(()=>snapshot.value.groups),review=computed(()=>snapshot.value.review);
</script>
<template>
  <details class="defense-build" @toggle="open=($event.target as HTMLDetailsElement).open">
    <summary>炮塔属性、技能规则与战斗统计 <span>· 点击{{ open?'收起':'展开' }}</span></summary>
    <div v-if="open" class="defense-details-body">
      <section v-for="group in groups" :key="group.name"><h4>{{ group.name }}</h4><dl><template v-for="row in group.rows" :key="row[0]"><dt>{{ row[0] }}</dt><dd>{{ row[1] }}</dd></template></dl></section>
      <section class="defense-statistics"><h4>本局实际战斗统计</h4><p>从第{{ defenseFeatures(state).log.startWave }}波接入统计后开始记录 · 已记录 {{ defenseNumber(review.seconds) }}秒 · 平均有效伤害 {{ defenseNumber(review.dps) }}/秒</p>
        <div v-for="entry in review.damage" :key="entry.id" class="defense-damage-row"><span>{{ entry.name }}</span><i><em :style="{width:`${entry.share*100}%`}" /></i><b>{{ defenseNumber(entry.value) }} · {{ (entry.share*100).toFixed(1) }}%</b></div>
        <p>主要受伤来源：{{ review.top }}。{{ review.suggestion }}</p>
        <div v-if="defenseFeatures(state).log.history.length" class="defense-wave-history"><table><thead><tr><th>最近波次</th><th>用时</th><th>有效伤害</th><th>承受伤害</th></tr></thead><tbody><tr v-for="wave in defenseFeatures(state).log.history.slice().reverse()" :key="wave.wave"><td>{{ wave.wave }}</td><td>{{ defenseNumber(wave.seconds) }}秒</td><td>{{ defenseNumber(wave.damage) }}</td><td>{{ defenseNumber(wave.taken) }}</td></tr></tbody></table></div>
      </section>
      <section class="defense-rule-list"><h4>限制与结算规则</h4><p v-for="rule in DEFENSE_RULES" :key="rule">{{ rule }}</p></section>
    </div>
  </details>
</template>
<style scoped>
.defense-build{margin-top:9px;border:1px solid var(--line);border-radius:9px;color:var(--text);font-size:11px}.defense-build summary{padding:10px 12px;cursor:pointer;font-weight:600;line-height:1.6}.defense-build summary span{color:var(--muted);font-weight:400}.defense-details-body{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:12px;padding:0 12px 12px}.defense-details-body section{min-width:0;border-top:1px solid var(--line);padding-top:9px}h4{margin:0 0 8px;font-size:12px;color:var(--primary)}dl{display:grid;grid-template-columns:minmax(95px,1fr) minmax(0,1.5fr);gap:7px 12px;margin:0;line-height:1.6}dt{color:var(--muted)}dd{margin:0;overflow-wrap:anywhere;font-variant-numeric:tabular-nums}p{font-size:11px;color:var(--muted);line-height:1.8;margin:6px 0}.defense-statistics,.defense-rule-list{grid-column:1/-1}.defense-damage-row{display:flex;align-items:center;gap:9px;margin:7px 0;font-size:10px}.defense-damage-row>span{width:64px}.defense-damage-row i{flex:1;height:5px;border-radius:3px;background:var(--line);overflow:hidden}.defense-damage-row em{height:100%;display:block;background:var(--primary)}.defense-damage-row b{min-width:120px;text-align:right;font-weight:400;font-variant-numeric:tabular-nums}.defense-wave-history{overflow-x:auto}table{width:100%;border-collapse:collapse;font-size:10px;text-align:left}th,td{padding:6px;border-bottom:1px solid var(--line)}th{color:var(--muted);font-weight:500}
@media(max-width:650px){.defense-details-body{grid-template-columns:1fr}.defense-damage-row b{min-width:90px}dl{grid-template-columns:1fr 1.5fr}}
</style>
