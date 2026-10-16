import { DEFENSE_CARDS, DEFENSE_CORES, DEFENSE_EVOLUTIONS, defenseCardRequirements, defenseChoiceLevel, defenseFeatures, defenseLevel, defenseRegion, defenseStats, defenseTier, type DefenseState } from './defense';
import { DEFENSE_ENEMY_NAMES } from './defense';
const numberFormat=new Intl.NumberFormat('zh-CN',{maximumFractionDigits:2});
export const defenseNumber=(n:number)=>Math.abs(n)>=1e7?n.toExponential(2):numberFormat.format(n);
const pct=(n:number)=>defenseNumber(n*100)+'%';
// 说明与规则使用同一组实时属性，等级、触发条件和数值不另写一套上限。
const details:Record<string,string>={
  power:'每级增加基础攻击的22%，与攻击、穿甲等基础加成相加；直击、溅射、持续伤害均随攻击力成长。',
  speed:'每级增加基础攻速的16%。攻速没有每秒18次的旧上限，高频命中按完整次数合并结算。',
  crit:'每级增加9个百分点暴击率。超过100%的部分继续提高暴击倍率；基础暴击为2倍。',
  multi:'每级增加1个同时直击目标，不再限制6个。每轮对每个直击目标命中1次，目标不足时只打存活目标。',
  pierce:'每级增加基础攻击的8%；重甲的42%基础减伤按1÷（1＋等级×0.4）递减，升级持续有效。',
  execute:'目标生命低于30%时，每级额外增加20%直击伤害，对首领也生效；按命中时生命判断。',
  focus:'优先攻击射程内的首领，每级额外增加25%对首领的直击伤害；没有首领时仍攻击普通敌人。',
  range:'每级增加14点攻击半径，不再限制300。射程超过300后，额外按射程÷300提高伤害，保留旧无尽的远程聚焦收益。',
  overclock:'每级增加基础攻速的25%，并减少5点最大生命。最大生命最低1点；攻速继续按完整次数结算。',
  blast:'命中后对其他敌人溅射，基础半径34，伤害比例为30%＋每级10%＋扩散等级×4%。主目标不重复受溅射。',
  radius:'每级扩大爆炸半径9点并增加4个百分点的溅射比例。需要爆破弹或重炮塔芯；范围不设玩法上限。',
  chain:'命中后电弧造成原直击伤害45%的伤害；首级跳2个其他目标，之后每级加1个，不再限制8个目标。',
  links:'每级增加1个连锁目标和15点连锁距离。需要连锁闪电或电磁塔芯；实际跳向范围内存活的其他敌人。',
  burn:'命中后附加3秒灼烧，每秒造成攻击力×13%×等级的伤害。再次命中刷新灼烧，不重复叠加同种灼烧层。',
  inferno:'每级延长灼烧1秒，并扩大死亡传播半径5点；燃烧目标死亡时传播3秒灼烧，需要燃烧弹。',
  slow:'命中减速2.5秒，减速比例为等级×12%÷（1＋等级×12%）。效果持续成长，不会使敌人永久无法移动。',
  freeze:'每级增加12个百分点冻结概率，达到100%后每次均触发；每级还延长0.08秒，首领只承受40%冻结时长。',
  storm:'电弧击中冻结目标时，每级增加40%连锁伤害。需要连锁闪电与寒冰封锁；可继续成长并解锁碎冰进化。',
  health:'每级增加40点最大生命，并立即恢复40点生命。实际回复不超过新生命上限，已损失生命不会被重置。',
  shield:'每级增加35点最大护盾，选择时立即补满。伤害先由护盾吸收；回充效果由护盾回充技能提供。',
  armor:'每级增加0.09护甲系数。实际承伤＝原伤害÷（1＋总护甲系数），收益递减但不再设72%硬上限。',
  regen:'每级额外每秒恢复0.8点生命，与金币购买的维修叠加；实际治疗不超过生命上限，暂停时不补算回复。',
  thorns:'距离塔35以内的敌人攻击时，每级反弹敌人原攻击力80%的伤害。远处攻击不触发，反伤可暴露组合进化。',
  barrier:'每级每秒恢复1.4点护盾，持续生效。腐蚀者命中后3秒内回充效率减半；可以配合护盾破裂反击。',
  nova:'冲击波每次造成攻击力×（1＋等级×50%）伤害。每级增加5点半径并提高触发频率；推开敌人18点。',
  leech:'每次有效命中按实际造成伤害的0.3%×等级恢复生命，直击、溅射、闪电与进化均可触发；不按过量伤害回血。',
  fortress:'生命低于50%时，每级增加基础攻击20%和0.08护甲系数。护甲采用递减公式，脱离低生命状态后加成结束。',
  income:'每级增加15个百分点的击杀金币收益，与基础收益升级相加。击杀奖励按实际死亡结算一次，不会重复发放。',
  interest:'每波结束按当时金币的2.5%×等级获得利息，向下取整；取消原每波75金币上限，可配合自动升级预留金币。',
  bounty:'每级使首领奖励系数增加2倍，重甲、护盾兵和护卫者增加0.25倍。奖励以普通敌人的基础金币为口径。',
  discount:'基础升级价格除以（1＋等级×10%），每级继续降低实际购买费用；最终价格最低1金币，不产生负价格。',
  healKill:'每击杀1个敌人，每级恢复1.4点生命；需要实际击杀且不超过生命上限，分裂子体也按实际击杀计算。',
  supply:'每守住1波，每级额外恢复8点生命并增加5金币。强化奖励累计，下一波自动开始，不强迫立即选卡。',
  growth:'每级增加当前波数×0.5%的基础攻击加成。波数增长时实时提高攻击，进入无尽后继续有效。',
  fortune:'每级增加2.5个百分点暴击率，且每次暴击命中每级额外获得1金币；合并攻击仍按实际暴击次数结算。',
  reserve:'生命低于30%时周期补盾，每次补充15×等级点。周期为8÷（1＋等级×4%）秒，暂停时不计时。',
};
export function defenseCardDetails(state:DefenseState,id:string){
  const current=defenseChoiceLevel(state,id),stats=defenseStats(state),evolution=DEFENSE_EVOLUTIONS.find(e=>'evo:'+e.id===id);
  const next:DefenseState={...state,cards:{...state.cards}};
  if(evolution)next.features={...defenseFeatures(state),evolutions:{...defenseFeatures(state).evolutions,[evolution.id]:current+1}};
  else next.cards[id]=current+1;
  const after=defenseStats(next);
  const changes=[
    ...(after.damage!==stats.damage?['单发 '+defenseNumber(stats.damage)+' → '+defenseNumber(after.damage)]:[]),
    ...(after.rate!==stats.rate?['攻速 '+defenseNumber(stats.rate)+' → '+defenseNumber(after.rate)+'/秒']:[]),
    ...(after.range!==stats.range?['射程 '+defenseNumber(stats.range)+' → '+defenseNumber(after.range)]:[]),
    ...(after.maxHP!==stats.maxHP?['生命 '+defenseNumber(stats.maxHP)+' → '+defenseNumber(after.maxHP)]:[]),
  ];
  return{description:(evolution?.description||details[id]||DEFENSE_CARDS.find(c=>c.id===id)?.description||'')+(evolution?' 下一等级：'+defenseEvolutionEffect(state,evolution.id,current+1):'')+' 当前'+current+'级，下一等级'+(current+1)+'级。',
    context:'当前单发 '+defenseNumber(stats.damage)+' · 攻速 '+defenseNumber(stats.rate)+'/秒 · 生命 '+defenseNumber(stats.maxHP),
    current,next:changes.join(' · '),requirements:defenseCardRequirements(state,id)};
}
export function defenseEvolutionEffect(s:DefenseState,id:string,rank:number){
  const x=defenseStats(s);
  if(id==='firestorm')return '每次燃烧击杀爆炸造成'+defenseNumber(x.damage*(.75+rank*.4))+'伤害，半径'+defenseNumber(65+rank*12)+'。';
  if(id==='froststorm')return '电弧触发碎冰，造成原直击的'+pct(.16+rank*.1)+'伤害，半径'+defenseNumber(45+rank*10)+'。';
  if(id==='bulwark')return '护盾破裂反击'+defenseNumber(x.damage*(1+rank*.8)+x.maxShield*rank*.35)+'伤害，半径'+defenseNumber(125+rank*14)+'，冷却'+defenseNumber(2/(1+rank*.12))+'秒。';
  return '每12轮射击触发一次，伤害'+defenseNumber(x.damage*(.7+rank*.4))+'，覆盖半角'+defenseNumber((.32+rank*.08)*180/Math.PI)+'度（达到180度后覆盖整圈），射程同炮塔。';
}
export function defensePropertyGroups(s:DefenseState){
  const x=defenseStats(s),f=defenseFeatures(s),c=(id:string)=>s.cards[id]||0;
  const core=DEFENSE_CORES.find(c=>c.id===f.core),evolutions=DEFENSE_EVOLUTIONS.filter(e=>f.evolutions[e.id]);
  return [
    {name:'炮塔与直击',rows:[['塔芯',core?.name||'旧版标准塔芯'],['成长等级',defenseNumber(defenseLevel(s))+' · 外观阶段 '+defenseTier(s)+'/5'],
      ['单发攻击',defenseNumber(x.damage)],['攻击速度',defenseNumber(x.rate)+' 轮/秒'],['同时直击目标',defenseNumber(x.multi)],
      ['攻击半径',defenseNumber(x.range)],['远程聚焦伤害倍率',defenseNumber(x.rangeFocus)+'倍'],['暴击概率',pct(x.crit)],['暴击伤害',defenseNumber(x.critMultiplier)+'倍'],
      ['低血斩杀加成',pct(c('execute')*.2)],['首领直击加成',pct(c('focus')*.25)]]},
    {name:'技能与覆盖',rows:[['溅射伤害',x.blast?pct(.3+x.blast*.1+c('radius')*.04):'未解锁'],['爆炸半径',x.blast?defenseNumber(x.blastRadius):'未解锁'],
      ['连锁目标',defenseNumber(x.chain)],['连锁距离',defenseNumber(x.chainRange)],['连锁伤害',x.chain?'原直击的45%':'未解锁'],
      ['灼烧每秒伤害',defenseNumber(x.damage*x.burnDPS)],['灼烧时长',defenseNumber(x.burnDuration)+'秒'],
      ['减速效果',pct(x.slow)],['冻结概率 / 普通时长',pct(x.freezeChance)+' / '+defenseNumber(x.freezeDuration)+'秒'],
      ['震荡波周期 / 半径',c('nova')?defenseNumber(x.novaPeriod)+'秒 / '+defenseNumber(x.novaRadius):'未解锁'],
      ['组合进化',evolutions.map(e=>e.name+' '+f.evolutions[e.id]+'级').join('、')||'达到配方后可在强化面板选择']]},
    {name:'塔身与恢复',rows:[['当前 / 最大生命',defenseNumber(s.hp)+' / '+defenseNumber(x.maxHP)],['当前 / 最大护盾',defenseNumber(s.shield)+' / '+defenseNumber(x.maxShield)],
      ['实际减伤',pct(x.armor)],['每秒生命回复',defenseNumber(x.regen)],['每秒护盾回复',defenseNumber(x.shieldRegen)],
      ['命中吸血',pct(x.lifeSteal)],['近身反伤',pct(x.thorns)],['护盾干扰剩余',defenseNumber(f.disruption)+'秒']]},
    {name:'组合进化',rows:DEFENSE_EVOLUTIONS.map(e=>[e.name,f.evolutions[e.id]?f.evolutions[e.id]+'级 · '+defenseEvolutionEffect(s,e.id,f.evolutions[e.id]!):'未解锁 · '+defenseCardRequirements(s,'evo:'+e.id).join('、')])},
    {name:'成长与当前波次',rows:[['击杀金币倍率',defenseNumber(x.coinBoost)+'倍'],['升级价格倍率',defenseNumber(x.discount)+'倍'],
      ['每波利息',pct(.025*c('interest'))+'，无75金币旧上限'],['每波额外补给',defenseNumber(c('supply')*5)+'金币 / '+defenseNumber(c('supply')*8)+'生命'],
      ['区域阶段',defenseRegion(s).name+' · 第'+defenseRegion(s).stage+'阶段'],['援军待入场',defenseNumber(f.reinforcements.reduce((a,b)=>a+b.count,0))]]},
  ];
}
export const DEFENSE_RULES=[
  '两种模式的基础升级、强化牌和组合进化均不设等级上限；无尽波次持续推进，原存档与永久研究保留。',
  '攻速、射程、多目标数与连锁目标数取消原18次/秒、300半径、6目标和8连锁硬上限。实际只命中在场、存活且在对应范围内的目标。',
  '实际射程超过300后，还会按射程÷300提高全局伤害，保留旧无尽存档已获得的远程聚焦收益；这里是额外加成阈值，不是射程上限。',
  '同一轮直击不会重复选中同一目标；连锁不会回跳本轮的主目标。爆破、连锁、灼烧与进化按各自条件独立结算。',
  '暴击概率最高100%，溢出继续提高暴击倍率；冻结概率达到100%后每次命中均触发，继续升级仍延长冻结。首领冻结时长为普通敌人的40%。',
  '护甲与减速采用递减收益公式，升级持续有效。生命和护盾回复不超过各自上限，最大生命至少1点，升级价格至少1金币。',
  '强化次数逐波累计。默认免费无限刷新三选一，也可搜索全技能；前置条件未满足时会显示原因。剩余次数大于0时可连续选择，关闭面板后继续战斗。',
  '高频攻击与技能按完整次数合并结算，特效合并展示不会减少伤害。同屏敌人分批入场，待入场敌人和召唤援军不会被省略。',
  '远射者与腐蚀者每次射击后会向塔前压10点距离，最终进入较近射程；提高射程仍可提前击杀，低射程也不会因此永久卡波。',
  '所有数值受计算机安全精度保护。战斗统计只记录接入新统计后的实际有效伤害，不补造旧局历史；最近保留12波记录。',
  '窗口失去焦点继续战斗；手动暂停、隐藏窗口、强化选择或关闭续玩等待期间不计算战斗，也不补算离开时间。',
];
