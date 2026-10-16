<script setup lang="ts">
import { computed } from 'vue';
import type { LegionActor } from '../../utils/rest/legion';
const props = withDefaults(defineProps<{ kind: LegionActor['kind']; enemy?: boolean; motion?: number; walking?: boolean; clock?: number; tier?: number }>(), { motion: 0, clock: 0, tier: 1 });
const heavy = computed(() => ['giant','warlord','behemoth'].includes(props.kind));
const caster = computed(() => ['mage','priest','shaman','necromancer'].includes(props.kind));
const bow = computed(() => ['archer','skeleton'].includes(props.kind));
const hood = computed(() => ['archer','mage','priest','shaman','necromancer'].includes(props.kind));
const colors = computed(() => {
  if (props.enemy) return { coat: props.kind==='necromancer'?'#695786':'#9d5551', light:'#dcb09a', trim:'#eab07a', metal:'#968a83', dark:'#493e4e', cape:'#624353', skin:['goblin','swarm'].includes(props.kind)?'#9fb879':'#d6c4aa', gem:'#efa7b7' };
  const coats = {guard:'#4f88a7',sword:'#587596',archer:'#508a77',mage:'#7771ac',priest:'#ddd2ad',giant:'#719f9f'};
  return { coat: coats[props.kind as keyof typeof coats]??'#508a77', light:'#d9e9e8', trim:'#e3c37e', metal:'#a3c1cf', dark:'#354b67', cape:props.kind==='mage'?'#4a4a80':props.kind==='archer'?'#355e59':'#3c5e7e', skin:'#edcba2', gem:props.kind==='priest'?'#a0e6b3':props.kind==='mage'?'#b6b8ff':'#86dae1' };
});
const gait = computed(() => props.walking ? Math.sin(props.clock * 10) : 0);
const strike = computed(() => Math.sin(Math.min(1,Math.max(0,props.motion))*Math.PI));
const bodyPose = computed(() => `translate(${strike.value*3} ${Math.abs(gait.value)*-.9}) rotate(${strike.value*(heavy.value?6:3)} 0 -20)`);
const weaponPose = computed(() => `rotate(${strike.value*(caster.value?-12:bow.value?-7:48)} 9 -26)`);
</script>
<template>
  <g class="legion-figure" :data-legion-figure="kind" :transform="enemy?'scale(-1 1)':undefined" stroke="#283b49" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
    <ellipse cy="2" :rx="heavy?23:17" :ry="heavy?5:3.6" fill="#132d3852" stroke="none"/>
    <g v-if="kind==='runner'" class="legion-motion" :transform="bodyPose">
      <path d="m-20-12-9-8 5-9 10 8M-13-12l-5 13h6l8-13m16-1 4 14h7l-4-20" fill="#756c79"/>
      <path d="m-20-20 12-12 22 4 11 12-10 7-27-1Z" fill="#96929a"/>
      <path d="m7-28 1-15 9 8 6-2 6 13 11 5-6 7-15-1Z" fill="#b4a6a0"/>
      <path d="m-5-30 7 12-9-4-5 10-3-17Z" fill="#6c6273"/><path d="M24-25h3" stroke="#ffdb8c" stroke-width="3"/>
      <path d="m29-14 1 4 4-5" fill="#f1e4ce" stroke-width="1"/>
    </g>
    <g v-else-if="kind==='swarm'" class="legion-motion" :transform="bodyPose">
      <path d="m-6-24-22-13 4 17-7 8 25-2m12-10 22-13-4 17 7 8-25-2" fill="#7e6e85"/>
      <path d="m-4-25-5-13 11 7 8-6 1 14 4 9-6 10H-5l-9-9Z" fill="#8eaa7f"/>
      <path d="M-5-22h2m7 0h2" stroke="#ffe7a0" stroke-width="3"/><path d="m-3-13 3 3 4-3" fill="none"/>
      <path d="m-6-6-5 6m17-6 5 6" stroke="#c6b69a"/>
    </g>
    <g v-else :transform="heavy?'scale(1.28)':undefined">
      <g class="legion-motion" :transform="bodyPose">
        <path v-if="tier>=2||caster||kind==='warlord'" d="m-9-33-9 28 10-3 6 4 8-7-2-22Z" :fill="colors.cape"/>
        <g class="legion-motion" :transform="`rotate(${gait*18} -5 -12)`"><path d="m-9-16-2 16h9l2-16Z" :fill="colors.dark"/><path d="M-11-4h9v5h-12v-3Z" :fill="colors.metal"/><path d="M-9-9h6" stroke="#e8dfc0" stroke-width="1"/></g>
        <g class="legion-motion" :transform="`rotate(${-gait*18} 5 -12)`"><path d="m2-16 2 16h9l-3-16Z" :fill="colors.dark"/><path d="M4-4h9l3 4v1H4Z" :fill="colors.metal"/></g>
        <template v-if="kind==='giant'||kind==='behemoth'">
          <path d="M-13-33-23-25-22-11-14-9m27-24 12 8-2 14-9 2" :fill="colors.metal"/>
          <path d="M-14-31-7-36H8l9 9-5 16h-25Z" :fill="colors.coat"/><path d="m-9-29 10-3 10 3-3 12H-8Z" :fill="colors.metal"/>
          <path d="m-8-42 8-6 10 7-1 11-9 5-9-5Z" :fill="colors.metal"/><path d="M-5-37h3m5 0h3" :stroke="colors.gem" stroke-width="2.7"/>
          <path d="M-6-29h13M0-30v13" :stroke="colors.light" stroke-width="1" opacity=".7"/><path d="m-4-24 4-5 5 5-5 5Z" :fill="colors.gem"/>
          <path v-if="enemy" d="m-18-29-8-8 3 13m41-5 8-8-3 13M-4-45l4-8 5 8" fill="#d7c1a0"/>
          <g class="legion-motion" :transform="weaponPose"><path d="M17-25 27-33l6 7-7 17-12-4Z" :fill="colors.metal"/><path d="m23-25 6-3-4 9" fill="none" :stroke="colors.light"/></g>
        </template>
        <template v-else>
          <path d="m-9-32-4 20 12 4 12-5-3-20Z" :fill="colors.coat"/>
          <path d="m-7-31 6 5 7-5-1 15H-7Z" :fill="kind==='skeleton'?'#554856':colors.metal" :opacity="hood ? .25 : 1"/>
          <path d="M-11-16h20v5h-20Z" :fill="colors.dark"/><rect x="-2" y="-16" width="5" height="4" rx="1" :fill="colors.trim" stroke-width="1"/>
          <path v-if="tier>=3" d="M-9-30h17m-16 2v7m16-7v7" :stroke="colors.trim" stroke-width="2"/>
          <path v-if="kind==='skeleton'" d="M-5-28h10m-10 5h10M0-31v17" stroke="#d2c6b3" stroke-width="2"/>
          <path v-if="kind==='archer'" d="m-12-27-7-15 7-3 6 13" :fill="colors.dark"/><path v-if="kind==='archer'" d="m-16-38-2-10m6 10-2-11" stroke="#ddcca3"/>
          <circle cy="-39" r="9" :fill="kind==='skeleton'||kind==='necromancer'?'#d9d0bc':colors.skin"/>
          <path v-if="hood" d="M-11-35q-3-18 11-19 13 1 12 19l-7-10-7 1-4 9Z" :fill="colors.coat"/>
          <path v-if="kind==='mage'" d="m-10-48 9-19 6 16 10 6-24 1Z" :fill="colors.cape"/><path v-if="kind==='mage'" d="m-7-50 13-1" :stroke="colors.trim" stroke-width="2"/>
          <path v-if="kind==='priest'" d="m-7-45 7-15 8 15-7-3Z" :fill="colors.light"/><path v-if="kind==='priest'" d="M0-55v8m-3-4h6" :stroke="colors.trim" stroke-width="1.5"/>
          <path v-else-if="!hood&&!['goblin','skeleton'].includes(kind)" d="M-11-37v-9l10-7 12 7v10H3v-11l-5 3v13Z" :fill="colors.metal"/>
          <path v-if="kind==='sword'" d="m-3-52-5-10 11 2 2 8" fill="#cfa976"/><path v-if="kind==='goblin'" d="m-8-43-12-7 10 16m18-9 12-7-10 16" :fill="colors.skin"/>
          <path v-if="kind==='warlord'||kind==='armored'" d="m-10-46-7-9 2 13m23-4 7-9-2 13" fill="#d8c7a6"/>
          <path d="M3-39h1" :stroke="kind==='skeleton'||kind==='necromancer'?'#a78be1':'#2e3944'" stroke-width="2.6"/>
          <path v-if="tier>=4" d="m-2-23 3-4 3 4-3 3Z" :fill="colors.gem" stroke-width="1"/>
          <path d="m-10-28-5 9 7 3m14-12 8 9-4 3" :fill="colors.coat"/>
          <g class="legion-motion" :transform="weaponPose">
            <g v-if="kind==='guard'||kind==='armored'"><path d="m8-32 16 4-1 19-9 7-9-9Z" :fill="colors.metal"/><path d="m10-28 10 2-1 14-5 5-6-7Z" :fill="colors.coat" stroke-width="1"/><path d="M14-25v13m-4-8h9" :stroke="colors.trim" stroke-width="1.6"/><path v-if="tier>=4" d="m11-18 3-5 4 5-4 4Z" :fill="colors.gem" stroke-width="1"/></g>
            <g v-else-if="bow"><path d="M17-41q19 20 0 39l1-39 12 20-13 19" fill="none" :stroke="colors.trim" stroke-width="2.3"/><path d="M8-21h28m-5-3 5 3-5 3" stroke="#ecdfb5"/><path d="m8-22-5-5-8 2" :stroke="colors.skin" stroke-width="3"/></g>
            <g v-else-if="caster"><path d="M19-2v-42" :stroke="colors.trim" stroke-width="2.8"/><path d="m12-43 7 7 7-7-7-8Z" :fill="colors.dark"/><circle cx="19" cy="-43" r="4" :fill="colors.gem"/><path v-if="tier>=3" d="m11-48 8-6 8 6" fill="none" :stroke="colors.trim" stroke-width="1"/><circle v-if="motion>0" cx="19" cy="-43" :r="6+strike*3" :stroke="colors.gem" stroke-width="1" fill="none" :opacity="strike*.7"/></g>
            <g v-else><path d="M9-22 24-46l5 1 1 5-18 22Z" :fill="colors.light"/><path d="m13-24 13-18" stroke="#ffffff" stroke-width=".8"/><path d="m7-26 11 8m-8 1-4 5" :stroke="colors.trim" stroke-width="3"/></g>
          </g>
          <path v-if="tier>=5" d="m-12-31-6 8 8 2m19-10 7 8-7 2" :fill="colors.trim" stroke-width="1"/>
        </template>
      </g>
    </g>
  </g>
</template>
<style scoped>
.legion-motion{transform-box:view-box}
@media(prefers-reduced-motion:reduce){.legion-motion{transform:none}}
</style>
