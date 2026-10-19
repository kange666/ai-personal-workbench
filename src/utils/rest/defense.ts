import { clamp, finite, integer, object, random } from "./progression";
export const DEFENSE_CARDS = [
  ["power","重型弹头","攻击伤害 +22%","速射"],["speed","快速装填","攻击速度 +16%","速射"],["crit","弱点瞄准","暴击率 +9%","速射"],["multi","双重射击","增加一个同时攻击目标","速射"],["pierce","穿甲弹","降低装甲减伤并提高伤害","速射"],["execute","收割者","对低生命敌人造成额外伤害","速射"],["focus","首领猎手","优先攻击首领，伤害提高","速射"],["range","远程瞄具","攻击范围 +14","速射"],["overclock","超频核心","攻速大幅提高，最大生命略减","速射"],
  ["blast","爆破弹","攻击产生范围爆炸","爆破"],["radius","爆破扩散","扩大爆炸范围并提高溅射伤害","爆破"],["chain","连锁闪电","攻击后闪电跳向附近敌人","元素"],["links","电弧延展","增加连锁目标与距离","元素"],["burn","燃烧弹","命中附带持续灼烧","元素"],["inferno","余烬蔓延","灼烧更持久，击杀时引燃邻敌","元素"],["slow","冰霜弹","命中使敌人减速","元素"],["freeze","寒冰封锁","周期性冻结被击中的敌人","元素"],["storm","风暴共鸣","被冻结的敌人承受更多连锁伤害","元素"],
  ["health","加固塔身","最大生命增加并立即治疗","堡垒"],["shield","能量护盾","增加最大护盾并补充护盾","堡垒"],["armor","复合装甲","降低受到的伤害","堡垒"],["regen","维修模块","每秒恢复生命","堡垒"],["thorns","荆棘装甲","近身攻击者受到反伤","堡垒"],["barrier","护盾回充","护盾自动缓慢恢复","堡垒"],["nova","震荡核心","周期冲击波伤害并推开近敌","堡垒"],["leech","生命回路","攻击命中恢复少量生命","堡垒"],["fortress","坚守阵地","生命低于一半时护甲和伤害提高","堡垒"],
  ["aura","熔能护环","持续灼伤塔周敌人，伤害随攻击和最大生命成长","堡垒"],["pulse","环形脉冲","周期释放整圈能量，伤害随攻击和最大护盾成长","堡垒"],["lance","聚能光束","周期发射贯穿敌阵的直线光束","速射"],
  ["income","赏金协议","击杀金币增加","成长"],["interest","储蓄利息","每波结束按金币获得利息","成长"],["bounty","精英悬赏","首领与精英奖励增加","成长"],["discount","采购优惠","基础升级费用降低","成长"],["healKill","急救回收","击杀恢复生命","成长"],["supply","战地补给","每波结束额外治疗并获得金币","成长"],["growth","厚积薄发","随当前波数增加伤害","成长"],["fortune","幸运一击","暴击额外掉落金币","成长"],["reserve","应急储备","低生命时周期补充护盾","成长"],
] .map(([id,name,description,family])=>({id:id!,name:name!,description:description!,family:family!}));
export type EnemyKind="normal"|"swarm"|"runner"|"armored"|"splitter"|"healer"|"shooter"|"shielded"|"charger"|"warder"|"hexer"|"brood"|"leech"|"boss";
export const DEFENSE_ENEMY_NAMES:Record<EnemyKind,string>={normal:"游荡者",swarm:"蜂群",runner:"疾行者",armored:"重甲",splitter:"分裂体",healer:"修复者",shooter:"远射者",shielded:"护盾兵",charger:"冲锋兽",warder:"护卫者",hexer:"腐蚀者",brood:"育巢者",leech:"噬能者",boss:"区域首领"};
export const DEFENSE_UPGRADES=[{id:"attack",name:"攻击",text:"伤害 +24%"},{id:"rate",name:"攻速",text:"攻速 +13%"},{id:"health",name:"塔身",text:"生命上限 +25"},{id:"range",name:"射程",text:"射程 +8"},{id:"regen",name:"维修",text:"每秒恢复 +0.6"},{id:"income",name:"收益",text:"金币收益 +10%"}] as const;

import { DEFENSE_BOSSES, DEFENSE_CORES, DEFENSE_DAMAGE_NAMES, DEFENSE_EVOLUTIONS, DEFENSE_FAMILIES, DEFENSE_REGIONS, type DefenseBossId, type DefenseCoreId, type DefenseDamageKind, type DefenseEvolutionId, type DefensePreference } from "./defenseContent";
export { DEFENSE_BOSSES, DEFENSE_CORES, DEFENSE_DAMAGE_NAMES, DEFENSE_EVOLUTIONS, DEFENSE_REGIONS };
export type { DefenseCoreId, DefenseDamageKind, DefenseEvolutionId, DefensePreference };
export type UpgradeId = typeof DEFENSE_UPGRADES[number]["id"];
export const DEFENSE_ENEMY_LIMIT = 65;
const LIMIT = Number.MAX_SAFE_INTEGER;
const safe = (n: number) => Math.max(0, Math.min(LIMIT, Number.isFinite(n) ? n : LIMIT));
const distance2 = (a: {x:number;y:number}, b: {x:number;y:number}) => (a.x-b.x)**2+(a.y-b.y)**2;
const tower = {x:300,y:200};
export interface DefenseEnemy {
  id:number; kind:EnemyKind; x:number; y:number; hp:number; maxHP:number; speed:number; damage:number; shield:number;
  cooldown:number; burn:number; burnTime:number; slow:number; freeze:number;
  boss?:DefenseBossId; skillTime?:number; charge?:number;siege?:number;freezeImmunity?:number;
}
export interface DefenseTrace {x:number;y:number;life:number;color:string;id?:number;kind?:'shot'|'chain'|'burst'|'enemy';variant?:'kinetic'|'shell'|'fire'|'ice'|'plasma'|'shield'|'nova'|'volley'|'pulse'|'beam';fromX?:number;fromY?:number;radius?:number;amount?:number}
export interface DefenseWaveRecord {wave:number;seconds:number;damage:number;taken:number}
export interface DefenseLog {
  since:number;startWave:number;shots:number;criticals:number;damage:Record<DefenseDamageKind,number>;
  incoming:Partial<Record<EnemyKind,number>>;history:DefenseWaveRecord[];waveDamage:number;waveTaken:number;peakDPS:number;
}
export interface DefenseFeatures {
  version:1;core:DefenseCoreId;preference:DefensePreference;evolutions:Partial<Record<DefenseEvolutionId,number>>;
  auto:{enabled:boolean;priority:UpgradeId|'balanced';reserve:number;batch:1|5|10;cursor:number};
  autoTime:number;bulwarkTime:number;disruption:number;traceId:number;bossWave:number;
  volleyBursts?:number;
  reinforcements:{kind:EnemyKind;count:number;x:number;y:number}[];log:DefenseLog;
}
export interface DefenseState {
  version:2;seed:number;phase:"ready"|"playing"|"choice"|"won"|"lost";wave:number;mode:"campaign"|"endless";
  hp:number;shield:number;coins:number;score:number;kills:number;upgrades:Record<UpgradeId,number>;cards:Record<string,number>;
  offers:string[];pendingUpgrades:number;choiceResume?:"playing"|"won";enemies:DefenseEnemy[];spawnLeft:number;spawnTime:number;shotTime:number;
  time:number;waveTime:number;nextId:number;traces:DefenseTrace[];perk:"power"|"guard"|"wealth";settled:boolean;last:string;
  // 可选扩展兼容全部 v2 存档；旧局在继续时接入，不重建金币、技能、波数或研究。
  features?:DefenseFeatures;
}
export interface DefenseMeta {version:1;bestWave:number;wins:number;shards:number;talents:{power:number;guard:number;wealth:number}}
function createFeatures(core:DefenseCoreId, since=0, startWave=1):DefenseFeatures {
  return {version:1,core,preference:"全部",evolutions:{},auto:{enabled:false,priority:"balanced",reserve:0,batch:1,cursor:0},
    autoTime:0,bulwarkTime:0,disruption:0,traceId:0,bossWave:0,volleyBursts:0,reinforcements:[],
    log:{since,startWave,shots:0,criticals:0,damage:{direct:0,blast:0,chain:0,burn:0,thorns:0,nova:0,evolution:0},incoming:{},history:[],waveDamage:0,waveTaken:0,peakDPS:0}};
}
export function defenseFeatures(s:DefenseState):DefenseFeatures {return s.features||createFeatures('classic',s.time,s.wave);}
export function configureDefense(s:DefenseState,settings:{preference?:DefensePreference;auto?:Partial<DefenseFeatures['auto']>}){
  const f=ensureFeatures(s);
  if(settings.preference&&DEFENSE_FAMILIES.includes(settings.preference))f.preference=settings.preference;
  const a=settings.auto;
  if(a){
    if(typeof a.enabled==='boolean')f.auto.enabled=a.enabled;
    if(a.priority&&['balanced',...DEFENSE_UPGRADES.map(u=>u.id)].includes(a.priority))f.auto.priority=a.priority;
    if(a.batch&&[1,5,10].includes(a.batch))f.auto.batch=a.batch;
    if(typeof a.reserve==='number'&&Number.isFinite(a.reserve))f.auto.reserve=Math.floor(safe(a.reserve));
  }
}
function ensureFeatures(s:DefenseState):DefenseFeatures {
  if(!s.features){
    s.features=createFeatures('classic',s.time,s.wave);
    if(s.wave%10===0&&(s.enemies.some(e=>e.kind==='boss')||s.spawnLeft<Math.min(36,5+Math.ceil(s.wave*1.2))))s.features.bossWave=s.wave;
  }
  return s.features;
}
export function createDefenseMeta():DefenseMeta{return{version:1,bestWave:0,wins:0,shards:0,talents:{power:0,guard:0,wealth:0}};}
export function validDefenseMeta(v:unknown):v is DefenseMeta{return object(v)&&v.version===1&&[v.bestWave,v.wins,v.shards].every(n=>integer(n,0,LIMIT))&&object(v.talents)&&[v.talents.power,v.talents.guard,v.talents.wealth].every(n=>integer(n,0,5));}
export function createDefense(seed=Date.now()>>>0,mode:DefenseState['mode']='campaign'):DefenseState {
  return{version:2,seed,phase:"ready",wave:1,mode,hp:180,shield:45,coins:45,score:0,kills:0,
    upgrades:{attack:0,rate:0,health:0,range:0,regen:0,income:0},cards:{},offers:[],pendingUpgrades:0,
    enemies:[],spawnLeft:0,spawnTime:0,shotTime:0,time:0,waveTime:0,nextId:1,traces:[],perk:"guard",settled:false,
    last:"选择塔芯和起始天赋。强化可搜索、累计与连续选择，打开时暂停战斗。",features:createFeatures('rapid')};
}
export function selectDefenseCore(s:DefenseState,id:DefenseCoreId){
  if(!DEFENSE_CORES.some(c=>c.id===id)||(s.phase!=='ready'&&defenseFeatures(s).core!=='classic'))return false;
  const before=defenseStats(s);ensureFeatures(s).core=id;const after=defenseStats(s);
  s.hp=Math.min(after.maxHP,s.hp+Math.max(0,after.maxHP-before.maxHP));s.shield=Math.min(after.maxShield,s.shield+Math.max(0,after.maxShield-before.maxShield));return true;
}
export function defenseStats(s:DefenseState){
  const c=(id:string)=>s.cards[id]||0,u=s.upgrades,f=defenseFeatures(s),core=DEFENSE_CORES.find(x=>x.id===f.core);
  const maxHP=Math.max(1,safe(180+u.health*25+c("health")*40+(s.perk==="guard"?30:0)+(core?.health||0)-c("overclock")*5));
  const low=s.hp<maxHP/2?c("fortress"):0;
  const mastery=Object.values(s.cards).reduce((sum,n)=>safe(sum+Math.max(0,n-6)),0);
  const critRating=.05+c("crit")*.09+c("fortune")*.025;
  const armorRating=c("armor")*.09+low*.08;
  // 保留旧无尽存档已获得的远程聚焦收益，同时让真实射程继续增长。
  const range=safe(150+u.range*8+c("range")*14),rangeFocus=Math.max(1,range/300);
  const damage=safe(18*(1+u.attack*.24+c("power")*.22+c("pierce")*.08+c("growth")*.005*s.wave+low*.2)*(s.perk==="power"?1.1:1)*(core?.damage||1)*(1+mastery*.04)*rangeFocus);
  const maxShield=safe(45+c("shield")*35+(f.core==='tesla'?20:0)),fortification=f.core==='bastion'?1.2:1;
  return {maxHP,maxShield,damage,
    rate:safe(1.8*(1+u.rate*.13+c("speed")*.16+c("overclock")*.25)*(core?.rate||1)),
    range,rangeFocus,multi:safe(1+c("multi")),chain:c("chain")?safe(1+c("chain")+c("links")):f.core==='tesla'?1:0,
    chainRange:safe(90+c("links")*15),blast:c("blast")+(f.core==='cannon'?.65:0),blastRadius:safe(34+c("radius")*9),
    crit:Math.min(1,critRating),critMultiplier:safe(2+Math.max(0,critRating-1)),armor:armorRating/(1+armorRating),armorRating,
    regen:safe(1.5+u.regen*.6+c("regen")*.8+(f.core==='bastion'?.8:0)),
    shieldRegen:safe(c("barrier")*1.4*(f.disruption>0?.5:1)),coinBoost:safe(1+u.income*.1+c("income")*.15),
    slow:.65*c("slow")*.12/(1+c("slow")*.12),freezeChance:.45*c("freeze")/(3+c("freeze")),freezeDuration:.65+.75*c("freeze")/(4+c("freeze")),freezeProtection:2.5,bossFreezeProtection:4,
    burnDPS:safe(.13*c("burn")),burnDuration:safe(3+c("inferno")),novaRadius:safe(115+c("nova")*5),novaPeriod:5/(1+c("nova")*.08),
    auraDPS:safe((damage*(.18*c("aura")+(f.core==='bastion'?.2:0))+maxHP*(.004*c("aura")+(f.core==='bastion'?.006:0)))*fortification),auraRadius:safe(105+c("aura")*7),
    pulsePeriod:Math.max(1.6,4.8/(1+c("pulse")*.12)),pulseRadius:safe(140+c("pulse")*7),pulseDamage:safe((damage*(.75+c("pulse")*.45)+maxShield*c("pulse")*.1)*fortification),
    lancePeriod:Math.max(1.5,5/(1+c("lance")*.1)),lanceDamage:safe(damage*(1+c("lance")*.65)),lanceWidth:safe(10+c("lance")*2),fortification,
    lifeSteal:c("leech")*.003,discount:1/(1+c("discount")*.1),thorns:c("thorns")*.8+(f.core==='bastion'?.25:0),
  };
}
export function defenseLevel(s:DefenseState){return safe(Object.values(s.upgrades).reduce((a,b)=>safe(a+b),0)+Object.values(s.cards).reduce((a,b)=>safe(a+b),0));}
export function defenseTier(s:DefenseState){const level=defenseLevel(s);return level>=120?5:level>=60?4:level>=28?3:level>=10?2:1;}
export function defenseRegion(s:DefenseState){const stage=Math.floor((s.wave-1)/20);return{...DEFENSE_REGIONS[stage%DEFENSE_REGIONS.length]!,stage:stage+1};}
export function defenseWaveTheme(s:DefenseState){
  if(s.wave%10===0)return DEFENSE_BOSSES[(['ram','bulwark','summoner'] as const)[(Math.floor(s.wave/10)-1)%3]!].name;
  return ['交错来袭','蜂群突袭','疾行突破','装甲编队','支援护送'][Math.floor((s.wave-1)/2)%5]!;
}
export function defenseUpgradeCost(s:DefenseState,id:UpgradeId,level=s.upgrades[id]){
  const price=level>20?12*1.28**20*(level/20)**2:12*1.28**level;
  return Math.max(1,Math.round(Math.min(LIMIT,price)/(1+(s.cards.discount||0)*.1)));
}
export function defensePurchaseQuote(s:DefenseState,id:UpgradeId,count:number,reserve=0){
  let cost=0,levels=0;
  for(let n=0;n<Math.min(10,Math.max(1,Math.floor(count)));n++){
    if(!Number.isSafeInteger(s.upgrades[id]+n+1))break;
    const price=defenseUpgradeCost(s,id,s.upgrades[id]+n);
    if(cost+price>s.coins-reserve)break;cost+=price;levels++;
  }
  return{cost,levels};
}
export function upgradeDefense(s:DefenseState,id:UpgradeId){
  if(!["ready","playing","choice"].includes(s.phase)||!DEFENSE_UPGRADES.some(x=>x.id===id))return false;
  const cost=defenseUpgradeCost(s,id);if(s.coins<cost||!Number.isSafeInteger(s.upgrades[id]+1))return false;
  s.coins-=cost;s.upgrades[id]++;if(id==="health")s.hp=Math.min(defenseStats(s).maxHP,s.hp+25);return true;
}
export function purchaseDefenseLevels(s:DefenseState,id:UpgradeId,count:number,reserve=0){
  const quote=defensePurchaseQuote(s,id,count,reserve);let bought=0;
  for(;bought<quote.levels;bought++)if(!upgradeDefense(s,id))break;
  return bought;
}
function waveSize(s:DefenseState){return s.wave<=30?Math.min(40,5+Math.ceil(s.wave*1.2)):Math.min(LIMIT,40+Math.floor(Math.sqrt(s.wave-30)*3));}
export function startDefenseWave(s:DefenseState){
  const f=ensureFeatures(s);s.phase="playing";s.spawnLeft=waveSize(s);s.spawnTime=.2;s.waveTime=0;s.enemies=[];
  f.reinforcements=[];f.log.waveDamage=0;f.log.waveTaken=0;
  s.last='第 '+s.wave+' 波 · '+defenseWaveTheme(s)+' · '+defenseRegion(s).name;
}
export function startDefense(s:DefenseState,meta:DefenseMeta){
  if(s.phase!=="ready")return;const level=meta.talents[s.perk];
  if(s.perk==="power")s.upgrades.attack+=level;if(s.perk==="guard")s.upgrades.health+=level;if(s.perk==="wealth")s.coins+=25+level*8;
  const stats=defenseStats(s);s.hp=stats.maxHP;s.shield=stats.maxShield;startDefenseWave(s);
}
export function enterDefenseEndless(s:DefenseState){
  if(s.mode==='endless'||s.phase==='lost')return false;s.mode='endless';
  if(s.phase==='won'){s.wave=safe(s.wave+1);startDefenseWave(s);}
  else if(s.phase==='choice'&&s.choiceResume==='won'){
    const offers=s.offers;s.wave=safe(s.wave+1);startDefenseWave(s);s.phase='choice';s.choiceResume='playing';s.offers=offers;
  }
  return true;
}
export function defenseCardRequirements(s:DefenseState,id:string){
  const evolution=DEFENSE_EVOLUTIONS.find(e=>'evo:'+e.id===id);
  if(evolution){
    const rank=defenseFeatures(s).evolutions[evolution.id]||0;
    return Object.entries(evolution.requirements).filter(([key,n])=>(s.cards[key]||0)<n*(rank+1)).map(([key,n])=>(DEFENSE_CARDS.find(c=>c.id===key)?.name||key)+' '+n*(rank+1)+'级');
  }
  const dependencies:Record<string,string[]>={radius:['blast'],links:['chain'],inferno:['burn'],freeze:['slow'],storm:['chain','freeze']};
  return (dependencies[id]||[]).filter(key=>!s.cards[key]&&!(key==='blast'&&defenseFeatures(s).core==='cannon')&&!(key==='chain'&&defenseFeatures(s).core==='tesla')).map(key=>DEFENSE_CARDS.find(c=>c.id===key)!.name);
}
export function defenseChoiceLevel(s:DefenseState,id:string){return id.startsWith('evo:')?defenseFeatures(s).evolutions[id.slice(4) as DefenseEvolutionId]||0:s.cards[id]||0;}
export function defenseChoiceCatalog(s:DefenseState,query='',family:DefensePreference='全部'){
  const keywords:Record<string,string>={burn:'火 火焰 灼烧',inferno:'火 火焰 燃烧 灼烧',leech:'吸血 回血',regen:'治疗 回血',healKill:'治疗 回血',chain:'电 闪电 电弧',links:'电 闪电 电弧',freeze:'冰 冰霜 冰冻',slow:'冰 冰霜 冰冻',storm:'冰 电 闪电 冰霜 冰冻 冻结',nova:'范围 击退 冲击',aura:'火 持续 范围 光环',pulse:'圆形 范围 环形 护盾',lance:'穿透 激光 光束',thorns:'反弹 反击',speed:'攻速 射速 次数',overclock:'攻速 射速 次数','evo:firestorm':'火 燃烧 灼烧','evo:froststorm':'冰 电 闪电','evo:bulwark':'盾 反伤','evo:volley':'速射 穿透 攻速'};
  const words=query.trim().toLowerCase().slice(0,100).split(/\s+/).filter(Boolean);
  const entries=[...DEFENSE_CARDS,...DEFENSE_EVOLUTIONS.map(e=>({...e,id:'evo:'+e.id}))];
  return entries.filter(c=>(family==='全部'||c.family===family)&&words.every(w=>(c.name+' '+c.family+' '+c.description+' '+(keywords[c.id]||'')).toLowerCase().includes(w)))
    .map(c=>({...c,level:defenseChoiceLevel(s,c.id),requirements:defenseCardRequirements(s,c.id)}));
}
function choiceKnown(id:string){return DEFENSE_CARDS.some(c=>c.id===id)||DEFENSE_EVOLUTIONS.some(e=>'evo:'+e.id===id);}
export function defenseOffers(s:DefenseState,refresh=false){
  if(s.pendingUpgrades<1||!['playing','won','choice'].includes(s.phase))return false;
  if(s.phase!=='choice'){s.choiceResume=s.phase as 'playing'|'won';s.phase='choice';}
  const pool=defenseChoiceCatalog(s).filter(c=>!c.requirements.length&&Number.isSafeInteger(c.level+1));
  if(!refresh&&s.offers.length&&s.offers.every(id=>pool.some(c=>c.id===id)))return true;
  const previous=new Set(s.offers),preference=defenseFeatures(s).preference;
  const shuffled=[...pool];for(let i=shuffled.length-1;i>0;i--){const j=Math.floor(random(s)*(i+1));[shuffled[i],shuffled[j]]=[shuffled[j]!,shuffled[i]!];}
  shuffled.sort((a,b)=>Number(previous.has(a.id))-Number(previous.has(b.id)));
  const offers:string[]=[];
  const evolved=shuffled.find(c=>c.id.startsWith('evo:'));if(evolved)offers.push(evolved.id);
  if(preference!=='全部'){const preferred=shuffled.find(c=>c.family===preference&&!offers.includes(c.id));if(preferred)offers.push(preferred.id);}
  if(!refresh&&s.wave<=5){const starter=["power","speed","multi","blast"][Math.max(0,s.wave-2)]!;if(starter&&!offers.includes(starter))offers.push(starter);}
  for(const card of shuffled)if(offers.length<3&&!offers.includes(card.id))offers.push(card.id);
  s.offers=offers.slice(0,3);return s.offers.length>0;
}
export function closeDefenseOffers(s:DefenseState){if(s.phase!=='choice')return false;s.phase=s.choiceResume||'playing';delete s.choiceResume;return true;}
export function pickDefenseCard(s:DefenseState,id:string,fromCatalog=false){
  if(s.phase!=='choice'||s.pendingUpgrades<1||!choiceKnown(id)||(!fromCatalog&&!s.offers.includes(id))||defenseCardRequirements(s,id).length||!Number.isSafeInteger(defenseChoiceLevel(s,id)+1))return false;
  if(id.startsWith('evo:')){const key=id.slice(4) as DefenseEvolutionId;const f=ensureFeatures(s);f.evolutions[key]=(f.evolutions[key]||0)+1;}
  else s.cards[id]=(s.cards[id]||0)+1;
  const stats=defenseStats(s);if(id==='health')s.hp=Math.min(stats.maxHP,s.hp+40);if(id==='shield')s.shield=stats.maxShield;
  s.pendingUpgrades--;s.offers=[];
  if(s.pendingUpgrades>0)defenseOffers(s,true);else closeDefenseOffers(s);
  s.last='强化已获得，剩余 '+s.pendingUpgrades+' 次'+(s.pendingUpgrades?'，可以连续选择。':'，战斗继续。');return true;
}
function addTrace(s:DefenseState,trace:Omit<DefenseTrace,'life'|'id'>,priority=false){
  if(s.traces.length>=32){if(!priority)return;s.traces.shift();}
  const f=ensureFeatures(s);f.traceId=(f.traceId+1)%LIMIT;
  s.traces.push({...trace,id:f.traceId,life:.24});
}
function spawnEnemy(s:DefenseState,kind?:EnemyKind,x?:number,y?:number){
  if(s.enemies.length>=DEFENSE_ENEMY_LIMIT)return false;
  const pools:EnemyKind[][]=[['normal','swarm'],['swarm','swarm','runner'],['runner','normal','charger'],['armored','shielded','warder'],['healer','shooter','hexer','brood','leech']];
  const unlock:Record<EnemyKind,number>={normal:1,swarm:1,runner:3,armored:5,splitter:10,healer:13,shooter:11,shielded:7,charger:6,warder:14,hexer:18,brood:16,leech:20,boss:10};
  const available=(Object.keys(unlock) as EnemyKind[]).filter(k=>k!=='boss'&&s.wave>=unlock[k]);
  const preferred=pools[Math.floor((s.wave-1)/2)%5]!.filter(k=>s.wave>=unlock[k]);
  const pool=preferred.length&&random(s)<.6?preferred:available;
  kind=kind||pool[Math.floor(random(s)*pool.length)]!;
  const angle=random(s)*Math.PI*2,stage=Math.floor((s.wave-1)/20);
  const pressure=1+Math.min(.65,Math.max(0,s.wave-5)*.026)+Math.log2(stage+1)*.08;
  const base=safe((21+s.wave*2.8+s.wave*s.wave*.33)*pressure)/10;
  const scale=kind==='boss'?220+60*Math.log2(1+s.wave/10):kind==='swarm'?5:kind==='runner'?7:kind==='armored'?15:kind==='warder'?12:kind==='brood'?14:10;
  const hp=safe(base*scale),id=s.nextId;
  s.nextId=(s.nextId+1)%LIMIT;while(s.enemies.some(e=>e.id===s.nextId))s.nextId=(s.nextId+1)%LIMIT;
  const boss:DefenseBossId|undefined=kind==='boss'?(['ram','bulwark','summoner'] as const)[(Math.floor(s.wave/10)-1)%3]:undefined;
  s.enemies.push({id,kind,x:x??300+Math.cos(angle)*280,y:y??200+Math.sin(angle)*180,hp,maxHP:hp,
    speed:(kind==='boss'?6.5:kind==='runner'?22:kind==='charger'?14:kind==='swarm'?17:kind==='armored'||kind==='brood'?7:10)*(1.04+Math.min(.2,s.wave*.005)),
    damage:safe((kind==='boss'?17+s.wave*.3:4+s.wave*.16)*(1+stage*.04)),shield:kind==='shielded'||kind==='warder'?base*3.5:0,
    cooldown:0,burn:0,burnTime:0,slow:0,freeze:0,boss,skillTime:boss?1.5:3,charge:0});
  return true;
}
type Stats=ReturnType<typeof defenseStats>;
function hit(s:DefenseState,e:DefenseEnemy,amount:number,source:DefenseDamageKind,stats:Stats){
  if(e.hp<=0||amount<=0)return 0;
  if(e.kind==='armored')amount*=1-.42/(1+(s.cards.pierce||0)*.4);
  amount=safe(amount);const actual=Math.min(e.hp+e.shield,amount),absorbed=Math.min(e.shield,amount);
  e.shield=Math.max(0,e.shield-absorbed);e.hp=Math.max(0,e.hp-(amount-absorbed));
  const log=ensureFeatures(s).log;log.damage[source]=safe(log.damage[source]+actual);log.waveDamage=safe(log.waveDamage+actual);
  if(source!=='burn'&&source!=='thorns'&&stats.lifeSteal)s.hp=Math.min(stats.maxHP,s.hp+actual*stats.lifeSteal);
  return actual;
}
function incoming(s:DefenseState,e:DefenseEnemy,amount:number,stats:Stats){
  const f=ensureFeatures(s),before=s.shield;amount=safe(amount/(1+stats.armorRating));
  const actual=Math.min(s.hp+s.shield,amount),absorbed=Math.min(s.shield,amount);s.shield-=absorbed;s.hp=Math.max(0,s.hp-amount+absorbed);
  f.log.incoming[e.kind]=safe((f.log.incoming[e.kind]||0)+actual);f.log.waveTaken=safe(f.log.waveTaken+actual);
  if(f.evolutions.bulwark&&before>0&&s.shield<=0&&f.bulwarkTime<=0){
    const rank=f.evolutions.bulwark;f.bulwarkTime=2/(1+rank*.12);
    const radius=125+rank*14;for(const near of s.enemies)if(distance2(near,tower)<=radius*radius)hit(s,near,stats.damage*(1+rank*.8)+stats.maxShield*rank*.35,'evolution',stats);
    addTrace(s,{...tower,kind:'burst',variant:'shield',radius,color:'#bce3ae'});
  }
  if(e.kind==='hexer')f.disruption=3;if(e.kind==='leech')e.hp=Math.min(e.maxHP,e.hp+actual*.6);
  return actual;
}
function attack(s:DefenseState,volleys:number,stats:Stats){
  const f=ensureFeatures(s),enemies=s.enemies,positions=enemies.map(e=>({x:e.x,y:e.y})),n=enemies.length;
  const distances=positions.map(p=>distance2(p,tower));
  const targets=enemies.map((_,i)=>i).filter(i=>enemies[i]!.hp>0&&distances[i]!<=stats.range**2)
    .sort((a,b)=>((s.cards.focus&&enemies[a]!.kind==='boss')?-1e12:distances[a]!)-((s.cards.focus&&enemies[b]!.kind==='boss')?-1e12:distances[b]!));
  if(!targets.length)return;
  const matrix=stats.blast||stats.chain?positions.map(p=>positions.map(q=>distance2(p,q))):[];
  const before=f.log.shots;f.log.shots=safe(before+volleys);
  // 弹幕先于本轮普攻结算，避免目标先被普攻击杀而让齐射无声消失。
  const bursts=Math.floor((before+volleys)/12)-Math.floor(before/12),rank=f.evolutions.volley||0;
  if(rank&&bursts){
    f.volleyBursts=safe((f.volleyBursts||0)+bursts);
    const primary=enemies[targets[0]!]!,heading=Math.atan2(primary.y-200,primary.x-300),width=Math.min(Math.PI,.32+rank*.08);
    for(let i=0;i<n;i++)if(enemies[i]!.hp>0&&distances[i]!<=stats.range**2){
      const e=enemies[i]!,angle=Math.atan2(e.y-200,e.x-300),difference=Math.abs(Math.atan2(Math.sin(angle-heading),Math.cos(angle-heading)));
      if(difference<=width)hit(s,e,safe(stats.damage*bursts*(.7+rank*.4)),'evolution',stats);
    }
    // 固定7条齐射轨迹与一个炮口波纹，不随升级、攻速或敌人数增加绘制节点。
    const reach=Math.min(400,stats.range);
    addTrace(s,{...tower,kind:'burst',variant:'volley',radius:65,color:'#fff0b1',amount:bursts},true);
    for(let ray=0;ray<7;ray++){
      const angle=heading-width+ray*width/3;
      addTrace(s,{x:300+Math.cos(angle)*reach,y:200+Math.sin(angle)*reach,fromX:300,fromY:177,kind:'shot',variant:'volley',color:'#fff0b1',amount:bursts},true);
    }
  }
  // 次数不封顶。每步最多4批命中，溅射/电弧/碎冰按目标累加后结算，复杂度与攻速无关。
  const packets=volleys>4?1:volleys;
  let visuals=Math.max(0,32-s.traces.length);
  for(let packet=0;packet<packets;packet++){
    const count=Math.floor(volleys/packets)+(packet<volleys%packets?1:0);
    const splash=new Float64Array(n),arcs=new Float64Array(n),shatter=new Float64Array(n),ice=new Float64Array(n);
    for(const index of targets.slice(0,stats.multi)){
      const e=enemies[index]!;if(e.hp<=0)continue;
      const expected=count*stats.crit,crit=Math.floor(expected)+(random(s)<expected%1?1:0);
      let damage=safe(stats.damage*(count+crit*(stats.critMultiplier-1)));
      if(s.cards.execute&&e.hp/e.maxHP<.3)damage=safe(damage*(1+s.cards.execute*.2));
      if(s.cards.focus&&e.kind==='boss')damage=safe(damage*(1+s.cards.focus*.25));
      f.log.criticals=safe(f.log.criticals+crit);hit(s,e,damage,'direct',stats);
      if(crit&&s.cards.fortune)s.coins=safe(s.coins+crit*s.cards.fortune);
      const variant=s.cards.burn?'fire':s.cards.slow?'ice':f.core==='cannon'?'shell':f.core==='tesla'?'plasma':'kinetic';
      if(visuals>0){visuals--;addTrace(s,{x:e.x,y:e.y,fromX:300,fromY:177,kind:'shot',variant,amount:count,color:variant==='fire'?'#f4ad77':variant==='ice'?'#a2e5ef':variant==='plasma'?'#b9a6f4':'#f2d58c'});}
      if(s.cards.burn){e.burn=safe(stats.damage*stats.burnDPS);e.burnTime=stats.burnDuration;}
      if(s.cards.slow)e.slow=2.5;
      if(s.cards.freeze&&e.freeze<=0&&(e.freezeImmunity||0)<=0&&random(s)<1-(1-stats.freezeChance)**count){
        e.freeze=stats.freezeDuration*(e.kind==='boss'?.4:1);
        e.freezeImmunity=e.freeze+(e.kind==='boss'?stats.bossFreezeProtection:stats.freezeProtection);
      }
      if(stats.blast){
        for(let j=0;j<n;j++)if(j!==index&&enemies[j]!.hp>0&&matrix[index]![j]!<=stats.blastRadius**2)splash[j]=safe(splash[j]!+damage*(.3+stats.blast*.1+(s.cards.radius||0)*.04));
        if(visuals>0){visuals--;addTrace(s,{x:e.x,y:e.y,kind:'burst',variant:'shell',radius:stats.blastRadius,color:'#efb17c'});}
      }
      if(stats.chain){
        let used=0,origin=e;
        for(let j=0;j<n&&used<stats.chain;j++){
          const near=enemies[j]!;if(j===index||near.hp<=0||matrix[index]![j]!>stats.chainRange**2)continue;
          arcs[j]=safe(arcs[j]!+damage*.45*(near.freeze>0&&s.cards.storm?1+s.cards.storm*.4:1));used++;
          if(visuals>0){visuals--;addTrace(s,{x:near.x,y:near.y,fromX:origin.x,fromY:origin.y,kind:'chain',variant:'plasma',color:'#c2a5ff'});}origin=near;
          if(near.freeze>0&&f.evolutions.froststorm)shatter[j]=safe(shatter[j]!+damage*(.16+f.evolutions.froststorm*.1));
        }
      }
    }
    // 同轮的二次伤害视为同时到达；只按有效护盾/生命损失记账，避免重复统计过量伤害。
    for(let j=0;j<n;j++){const e=enemies[j]!;if(splash[j])hit(s,e,splash[j]!,'blast',stats);if(arcs[j])hit(s,e,arcs[j]!,'chain',stats);}
    const rank=f.evolutions.froststorm||0,radius=45+rank*10;
    if(rank)for(let j=0;j<n;j++)if(shatter[j]){
      for(let k=0;k<n;k++)if(enemies[k]!.hp>0&&matrix[j]![k]!<=radius**2)ice[k]=safe(ice[k]!+shatter[j]!);
      if(visuals>0){visuals--;addTrace(s,{x:positions[j]!.x,y:positions[j]!.y,kind:'burst',variant:'ice',radius,color:'#b3eaf5'});}
    }
    for(let j=0;j<n;j++)if(ice[j])hit(s,enemies[j]!,ice[j]!,'evolution',stats);
  }
}
function queueSpawn(s:DefenseState,kind:EnemyKind,count:number,x:number,y:number){
  const queue=ensureFeatures(s).reinforcements;
  const same=queue.find(p=>p.kind===kind);if(same)same.count=safe(same.count+count);else queue.push({kind,count,x,y});
}
function enemySkill(s:DefenseState,e:DefenseEnemy,dt:number,stats:Stats){
  e.skillTime=(e.skillTime??3)-dt;e.charge=Math.max(0,(e.charge||0)-dt);
  if(e.skillTime>0)return;
  const kind=e.boss||e.kind;
  if(kind==='summoner'||kind==='brood'){
    if(ensureFeatures(s).reinforcements.length||s.enemies.length>=DEFENSE_ENEMY_LIMIT)return;
    queueSpawn(s,'swarm',kind==='summoner'?3:1,kind==='summoner'?300+(e.x-300)*.5:e.x,kind==='summoner'?200+(e.y-200)*.5:e.y);e.skillTime=kind==='summoner'?6:8;
    addTrace(s,{x:e.x,y:e.y,kind:'burst',variant:'plasma',radius:26,color:'#d0a3e5'});
  }else if(kind==='bulwark'||kind==='warder'){
    for(const near of s.enemies)if(near.hp>0&&distance2(near,e)<85**2)near.shield=Math.min(near.maxHP*.5,near.shield+near.maxHP*.12);
    e.skillTime=5;addTrace(s,{x:e.x,y:e.y,kind:'burst',variant:'shield',radius:85,color:'#aad5ed'});
  }else if(kind==='healer'){
    for(const near of s.enemies)if(near.hp>0&&distance2(near,e)<65**2)near.hp=Math.min(near.maxHP,near.hp+near.maxHP*.05);
    e.skillTime=3;addTrace(s,{x:e.x,y:e.y,kind:'burst',variant:'shield',radius:65,color:'#bce2ac'});
  }else if(kind==='ram'||kind==='charger'){
    e.charge=2;e.skillTime=7;addTrace(s,{x:e.x,y:e.y,kind:'burst',variant:'fire',radius:28,color:'#f4ba8e'});
  }else e.skillTime=3;
}
export function settleDefense(s:DefenseState,meta:DefenseMeta){
  meta.bestWave=Math.max(meta.bestWave,s.phase==="won"||s.choiceResume==='won'?s.wave:s.wave-1);
  if(!s.settled&&(s.phase==="won"||s.phase==="lost")){meta.shards=safe(meta.shards+Math.floor((s.wave-1)/3));if(s.phase==="won"){meta.wins=safe(meta.wins+1);meta.shards=safe(meta.shards+5);}s.settled=true;}
}
export function defenseRunReview(s:DefenseState){
  const f=defenseFeatures(s),log=f.log,total=Object.values(log.damage).reduce((a,b)=>safe(a+b),0),seconds=Math.max(0,s.time-log.since);
  const sources=Object.entries(log.incoming).sort((a,b)=>b[1]!-a[1]!);
  const top=sources[0],name=top?DEFENSE_ENEMY_NAMES[top[0] as EnemyKind]:'暂无记录';
  const suggestion=top?.[0]==='boss'?'增加首领猎手、暴击或穿甲，并预留护盾与塔身应对首领波。':top&&['shooter','hexer','healer','warder'].includes(top[0])?'优先补充射程、穿甲或范围火力，处理远程与支援敌人。':top&&['swarm','splitter','brood'].includes(top[0])?'增加爆破、连锁或击杀恢复，降低密集敌人的近身压力。':'兼顾主力输出、塔身和维修，观察下一波的敌人组合。';
  return{total,seconds,dps:seconds?total/seconds:0,top:name,suggestion,sources,damage:Object.entries(log.damage).map(([id,value])=>({id,name:DEFENSE_DAMAGE_NAMES[id as DefenseDamageKind],value,share:total?value/total:0}))};
}
function areaSkills(s:DefenseState,dt:number,stats:Stats){
  const ticks=(period:number)=>Math.floor((s.time+1e-9)/period)-Math.floor((s.time-dt+1e-9)/period);
  if(stats.auraDPS){
    const count=ticks(.25);
    if(count)for(const e of s.enemies)if(e.hp>0&&distance2(e,tower)<=stats.auraRadius**2)hit(s,e,safe(stats.auraDPS*.25*count),'nova',stats);
  }
  if(s.cards.pulse){
    const count=ticks(stats.pulsePeriod);
    if(count){
      for(const e of s.enemies)if(e.hp>0&&distance2(e,tower)<=stats.pulseRadius**2)hit(s,e,safe(stats.pulseDamage*count),'nova',stats);
      addTrace(s,{...tower,kind:'burst',variant:'pulse',radius:stats.pulseRadius,color:'#a9f1d0'},true);
    }
  }
  if(s.cards.lance){
    const count=ticks(stats.lancePeriod);
    const target=s.enemies.filter(e=>e.hp>0&&distance2(e,tower)<=stats.range**2).sort((a,b)=>distance2(a,tower)-distance2(b,tower))[0];
    if(count&&target){
      const heading=Math.atan2(target.y-200,target.x-300),dx=Math.cos(heading),dy=Math.sin(heading);
      for(const e of s.enemies){const x=e.x-300,y=e.y-200,along=x*dx+y*dy;
        if(e.hp>0&&along>=0&&distance2(e,tower)<=stats.range**2&&Math.abs(x*dy-y*dx)<=stats.lanceWidth)hit(s,e,safe(stats.lanceDamage*count),'nova',stats);
      }
      const length=Math.min(400,stats.range);
      addTrace(s,{x:300+dx*length,y:200+dy*length,fromX:300,fromY:200,kind:'shot',variant:'beam',color:'#a8e9ff',radius:stats.lanceWidth},true);
    }
  }
}
export function stepDefense(s:DefenseState,dt:number){
  if(s.phase!=="playing"||!finite(dt,.000001,.2))return;
  const f=ensureFeatures(s),stats=defenseStats(s);
  if(s.hp<=0){s.phase='lost';return;}
  s.time=safe(s.time+dt);s.waveTime=safe(s.waveTime+dt);s.shotTime+=dt;f.bulwarkTime=Math.max(0,f.bulwarkTime-dt);f.disruption=Math.max(0,f.disruption-dt);
  for(const e of s.enemies){e.freeze=Math.max(0,e.freeze-dt);e.freezeImmunity=Math.max(0,(e.freezeImmunity||0)-dt);}
  s.hp=Math.min(stats.maxHP,s.hp+stats.regen*dt);s.shield=Math.min(stats.maxShield,s.shield+stats.shieldRegen*dt);
  f.autoTime+=dt;
  if(f.auto.enabled&&f.autoTime>=.5){
    f.autoTime%=.5;
    if(f.auto.priority==='balanced'){
      for(let n=0;n<DEFENSE_UPGRADES.length;n++){const id=DEFENSE_UPGRADES[f.auto.cursor%6]!.id;f.auto.cursor=(f.auto.cursor+1)%6;if(purchaseDefenseLevels(s,id,f.auto.batch,f.auto.reserve))break;}
    }else purchaseDefenseLevels(s,f.auto.priority,f.auto.batch,f.auto.reserve);
  }else if(f.autoTime>=.5)f.autoTime%=.5;
  if(s.cards.reserve&&s.hp<stats.maxHP*.3){
    const period=8/(1+s.cards.reserve*.04),count=Math.floor(s.time/period)-Math.floor((s.time-dt)/period);
    if(count)s.shield=Math.min(stats.maxShield,s.shield+15*s.cards.reserve*count);
  }
  s.spawnTime-=dt;
  const spawnBatch=s.wave>=8&&s.wave%10!==0?2:1;
  for(let spawn=0;spawn<spawnBatch&&s.spawnTime<=0&&s.enemies.length<DEFENSE_ENEMY_LIMIT;spawn++){
    const queued=f.reinforcements[0];
    if(queued){if(spawnEnemy(s,queued.kind,queued.x,queued.y)){queued.count--;if(!queued.count)f.reinforcements.shift();}}
    else if(s.spawnLeft>0){
      const boss=s.wave%10===0&&f.bossWave!==s.wave;
      if(spawnEnemy(s,boss?'boss':undefined)){s.spawnLeft--;if(boss)f.bossWave=s.wave;}
    }
    if(spawn===spawnBatch-1||s.enemies.length>=DEFENSE_ENEMY_LIMIT)s.spawnTime=Math.max(.24,.72-s.wave*.009);
  }
  const volleys=Math.floor(Math.min(LIMIT,s.shotTime*stats.rate)+1e-9);
  if(volleys>0){s.shotTime=Math.max(0,s.shotTime-volleys/stats.rate);attack(s,volleys,stats);}
  areaSkills(s,dt,stats);
  if(s.cards.nova){
    const count=Math.floor(s.time/stats.novaPeriod)-Math.floor((s.time-dt)/stats.novaPeriod);
    if(count){for(const e of s.enemies)if(e.hp>0&&distance2(e,tower)<stats.novaRadius**2){
      hit(s,e,safe(stats.damage*(1+s.cards.nova*.5)*stats.fortification*count),'nova',stats);
      const angle=Math.atan2(e.y-200,e.x-300),push=18*count;e.x=clamp(e.x+Math.cos(angle)*push,-400,1000);e.y=clamp(e.y+Math.sin(angle)*push,-400,800);
    }addTrace(s,{...tower,kind:'burst',variant:'nova',radius:stats.novaRadius,color:'#c8b6ef'});}
  }
  for(const e of [...s.enemies]){
    if(e.hp<=0)continue;e.cooldown-=dt;e.slow=Math.max(0,e.slow-dt);
    if(e.burnTime>0){const elapsed=Math.min(dt,e.burnTime);hit(s,e,e.burn*elapsed,'burn',stats);e.burnTime=Math.max(0,e.burnTime-dt);}if(e.hp<=0)continue;
    if(e.freeze>0)continue;enemySkill(s,e,dt,stats);
    // 远程打击后逐步前压，避免射程外敌人与高回复炮塔形成永不结束的僵局。
    const distance=Math.sqrt(distance2(e,tower)),ranged=e.kind==='shooter'||e.kind==='hexer';
    const reach=ranged?Math.max(30,(e.kind==='hexer'?265:235+Math.min(40,Math.max(0,s.wave-10)*2))-(e.siege||0)*10):24;
    if(distance>reach+1e-6){
      const speed=e.speed*(e.slow>0?1-stats.slow:1)*((e.charge||0)>0?2.6:1);
      const travel=Math.min(distance-reach,speed*dt);e.x+=(300-e.x)/distance*travel;e.y+=(200-e.y)/distance*travel;
    }else if(e.cooldown<=0){
      const damage=e.damage*((e.charge||0)>0?1.5:1);incoming(s,e,damage,stats);
      if(stats.thorns&&distance<35)hit(s,e,damage*stats.thorns,'thorns',stats);
      if(ranged){e.siege=safe((e.siege||0)+1);addTrace(s,{x:300,y:195,fromX:e.x,fromY:e.y,kind:'enemy',variant:e.kind==='hexer'?'plasma':'kinetic',color:e.kind==='hexer'?'#b5d981':'#efa5a0'});}
      e.cooldown=ranged?2.5:1.3;
    }
  }
  const dead=s.enemies.filter(e=>e.hp<=0);s.enemies=s.enemies.filter(e=>e.hp>0);
  for(const e of dead){
    s.kills=safe(s.kills+1);
    const gain=Math.round((3+s.wave*.25)*stats.coinBoost*(e.kind==='boss'?8+(s.cards.bounty||0)*2:['armored','shielded','warder'].includes(e.kind)?1+(s.cards.bounty||0)*.25:1));
    s.coins=safe(s.coins+gain);s.score=safe(s.score+10+s.wave*2);
    if(s.cards.healKill)s.hp=Math.min(stats.maxHP,s.hp+1.4*s.cards.healKill);
    if(e.kind==='splitter')queueSpawn(s,'swarm',2,e.x,e.y);
    if(e.burnTime>0){
      if(s.cards.inferno)for(const near of s.enemies)if(distance2(near,e)<(60+s.cards.inferno*5)**2){near.burn=Math.max(near.burn,e.burn);near.burnTime=Math.max(near.burnTime,3);}
      const rank=f.evolutions.firestorm||0,radius=65+rank*12;
      if(rank){for(const near of s.enemies)if(near.hp>0&&distance2(near,e)<radius**2)hit(s,near,stats.damage*(.75+rank*.4),'evolution',stats);
        addTrace(s,{x:e.x,y:e.y,kind:'burst',variant:'fire',radius,color:'#ffb378'});}
    }
  }
  s.traces=s.traces.map(t=>({...t,life:t.life-dt})).filter(t=>t.life>0).slice(-32);
  if(s.hp<=0){s.phase="lost";const review=defenseRunReview(s);s.last='本局主要受伤来源：'+review.top+'。'+review.suggestion;return;}
  if(s.spawnLeft===0&&s.enemies.length===0&&!f.reinforcements.length){
    const log=f.log;log.history.push({wave:s.wave,seconds:s.waveTime,damage:log.waveDamage,taken:log.waveTaken});if(log.history.length>12)log.history.shift();
    log.peakDPS=safe(Math.max(log.peakDPS,log.waveDamage/Math.max(.01,s.waveTime)));
    s.hp=Math.min(stats.maxHP,s.hp+stats.maxHP*.07+(s.cards.supply||0)*8);s.shield=Math.min(stats.maxShield,s.shield+8);
    s.coins=safe(s.coins+12+s.wave*2+(s.cards.supply||0)*5+Math.floor(s.coins*.025*(s.cards.interest||0)));
    s.pendingUpgrades=safe(s.pendingUpgrades+1);
    if(s.wave===30&&s.mode==="campaign"){s.phase="won";s.score=safe(s.score+1000);s.last="三十波守住了！可以继续挑战无尽。";}
    else{const cleared=s.wave;s.wave=safe(s.wave+1);startDefenseWave(s);s.last='第 '+cleared+' 波已守住，强化已累计 '+s.pendingUpgrades+' 次。';}
  }
}
function validFeatures(v:unknown){
  if(!object(v)||v.version!==1||!['classic',...DEFENSE_CORES.map(c=>c.id)].includes(String(v.core))||!DEFENSE_FAMILIES.includes(v.preference as DefensePreference)||!object(v.evolutions)||!object(v.auto)||!object(v.log))return false;
  if(!Object.entries(v.evolutions).every(([id,n])=>DEFENSE_EVOLUTIONS.some(e=>e.id===id)&&integer(n,0,LIMIT)))return false;
  const a=v.auto,l=v.log;
  if(typeof a.enabled!=='boolean'||!['balanced',...DEFENSE_UPGRADES.map(u=>u.id)].includes(String(a.priority))||![1,5,10].includes(Number(a.batch))||!integer(a.reserve,0,LIMIT)||!integer(a.cursor,0,5))return false;
  if(![v.autoTime,v.bulwarkTime,v.disruption,v.traceId,v.bossWave].every(n=>finite(n,0,LIMIT)))return false;
  if(v.volleyBursts!==undefined&&!integer(v.volleyBursts,0,LIMIT))return false;
  if(!Array.isArray(v.reinforcements)||v.reinforcements.length>32||!v.reinforcements.every(q=>object(q)&&Object.hasOwn(DEFENSE_ENEMY_NAMES,String(q.kind))&&integer(q.count,1,LIMIT)&&finite(q.x,-1000,1000)&&finite(q.y,-1000,1000)))return false;
  if(![l.since,l.startWave,l.shots,l.criticals,l.waveDamage,l.waveTaken,l.peakDPS].every(n=>finite(n,0,LIMIT))||!object(l.damage)||!object(l.incoming))return false;
  if(!Object.keys(DEFENSE_DAMAGE_NAMES).every(id=>finite((l.damage as Record<string,unknown>)[id],0,LIMIT))||!Object.entries(l.incoming).every(([id,n])=>Object.hasOwn(DEFENSE_ENEMY_NAMES,id)&&finite(n,0,LIMIT)))return false;
  return Array.isArray(l.history)&&l.history.length<=12&&l.history.every(h=>object(h)&&[h.wave,h.seconds,h.damage,h.taken].every(n=>finite(n,0,LIMIT)));
}
export function validDefense(v:unknown):v is DefenseState{
  if(!object(v)||v.version!==2||!integer(v.pendingUpgrades,0,LIMIT)||(v.choiceResume!==undefined&&!['playing','won'].includes(v.choiceResume as string))||(v.phase==='choice'&&(Number(v.pendingUpgrades)<1||v.choiceResume===undefined))||!integer(v.seed,0,4294967295)||!integer(v.wave,1,LIMIT)||!["ready","playing","choice","won","lost"].includes(String(v.phase))||!["campaign","endless"].includes(String(v.mode))||!["power","guard","wealth"].includes(String(v.perk)))return false;
  if(![v.hp,v.shield,v.coins,v.score,v.kills,v.spawnLeft,v.spawnTime,v.shotTime,v.time,v.waveTime,v.nextId].every(n=>finite(n,-LIMIT,LIMIT))||typeof v.settled!=="boolean"||typeof v.last!=="string"||!object(v.upgrades)||!DEFENSE_UPGRADES.every(u=>integer((v.upgrades as Record<string,unknown>)[u.id],0,LIMIT))||!object(v.cards)||!Object.entries(v.cards).every(([id,n])=>DEFENSE_CARDS.some(c=>c.id===id)&&integer(n,0,LIMIT)))return false;
  if(v.features!==undefined&&!validFeatures(v.features))return false;
  if(!Array.isArray(v.offers)||v.offers.length>3||new Set(v.offers).size!==v.offers.length||!v.offers.every(id=>typeof id==='string'&&choiceKnown(id))||!Array.isArray(v.enemies)||v.enemies.length>DEFENSE_ENEMY_LIMIT)return false;
  if(!v.enemies.every(e=>object(e)&&integer(e.id,0,LIMIT)&&Object.hasOwn(DEFENSE_ENEMY_NAMES,String(e.kind))&&[e.x,e.y,e.hp,e.maxHP,e.speed,e.damage,e.shield,e.cooldown,e.burn,e.burnTime,e.slow,e.freeze].every(n=>finite(n,-LIMIT,LIMIT))&&(e.boss===undefined||Object.hasOwn(DEFENSE_BOSSES,String(e.boss)))&&(e.skillTime===undefined||finite(e.skillTime,-1,LIMIT))&&(e.charge===undefined||finite(e.charge,0,LIMIT))&&(e.siege===undefined||integer(e.siege,0,LIMIT))))return false;
  if(!v.enemies.every(e=>e.freezeImmunity===undefined||finite(e.freezeImmunity,0,LIMIT)))return false;
  return Array.isArray(v.traces)&&v.traces.length<=70&&v.traces.every(t=>object(t)&&finite(t.x,-1000,1000)&&finite(t.y,-1000,1000)&&finite(t.life,0,1)&&typeof t.color==="string");
}
export function migrateDefense(v:unknown):DefenseState|null{
  if(!object(v)||v.version!==1)return null;
  const next={...v,version:2,pendingUpgrades:v.phase==='choice'?1:0,choiceResume:v.phase==='choice'?'playing':undefined,
    features:v.features?JSON.parse(JSON.stringify(v.features)):undefined} as DefenseState;
  if(!validDefense(next))return null;
  if(next.phase==='choice'){const offers=next.offers;next.wave++;startDefenseWave(next);next.offers=offers;delete next.choiceResume;}
  return next;
}
