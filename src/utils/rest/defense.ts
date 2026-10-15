import { clamp, finite, integer, object, random } from "./progression";
export const DEFENSE_CARDS = [
  ["power","重型弹头","攻击伤害 +22%","速射"],["speed","快速装填","攻击速度 +16%","速射"],["crit","弱点瞄准","暴击率 +9%","速射"],["multi","双重射击","增加一个同时攻击目标","速射"],["pierce","穿甲弹","降低装甲减伤并提高伤害","速射"],["execute","收割者","对低生命敌人造成额外伤害","速射"],["focus","首领猎手","优先攻击首领，伤害提高","速射"],["range","远程瞄具","攻击范围 +14","速射"],["overclock","超频核心","攻速大幅提高，最大生命略减","速射"],
  ["blast","爆破弹","攻击产生范围爆炸","爆破"],["radius","爆破扩散","扩大爆炸范围并提高溅射伤害","爆破"],["chain","连锁闪电","攻击后闪电跳向附近敌人","元素"],["links","电弧延展","增加连锁目标与距离","元素"],["burn","燃烧弹","命中附带持续灼烧","元素"],["inferno","余烬蔓延","灼烧更持久，击杀时引燃邻敌","元素"],["slow","冰霜弹","命中使敌人减速","元素"],["freeze","寒冰封锁","周期性冻结被击中的敌人","元素"],["storm","风暴共鸣","被冻结的敌人承受更多连锁伤害","元素"],
  ["health","加固塔身","最大生命增加并立即治疗","堡垒"],["shield","能量护盾","增加最大护盾并补充护盾","堡垒"],["armor","复合装甲","降低受到的伤害","堡垒"],["regen","维修模块","每秒恢复生命","堡垒"],["thorns","荆棘装甲","近身攻击者受到反伤","堡垒"],["barrier","护盾回充","护盾自动缓慢恢复","堡垒"],["nova","震荡核心","周期冲击波伤害并推开近敌","堡垒"],["leech","生命回路","攻击命中恢复少量生命","堡垒"],["fortress","坚守阵地","生命低于一半时护甲和伤害提高","堡垒"],
  ["income","赏金协议","击杀金币增加","成长"],["interest","储蓄利息","每波结束获得有上限的利息","成长"],["bounty","精英悬赏","首领与精英奖励增加","成长"],["discount","采购优惠","基础升级费用降低","成长"],["healKill","急救回收","击杀恢复生命","成长"],["supply","战地补给","每波结束额外治疗并获得金币","成长"],["growth","厚积薄发","随当前波数增加伤害","成长"],["fortune","幸运一击","暴击额外掉落金币","成长"],["reserve","应急储备","低生命时周期补充护盾","成长"],
] .map(([id,name,description,family])=>({id:id!,name:name!,description:description!,family:family!}));
export type EnemyKind="normal"|"swarm"|"runner"|"armored"|"splitter"|"healer"|"shooter"|"shielded"|"boss";
export const DEFENSE_ENEMY_NAMES:Record<EnemyKind,string>={normal:"游荡者",swarm:"蜂群",runner:"疾行者",armored:"重甲",splitter:"分裂体",healer:"修复者",shooter:"远射者",shielded:"护盾兵",boss:"区域首领"};
export const DEFENSE_UPGRADES=[{id:"attack",name:"攻击",text:"伤害 +24%"},{id:"rate",name:"攻速",text:"攻速 +13%"},{id:"health",name:"塔身",text:"生命上限 +25"},{id:"range",name:"射程",text:"射程 +8"},{id:"regen",name:"维修",text:"每秒恢复 +0.6"},{id:"income",name:"收益",text:"金币收益 +10%"}] as const;
type UpgradeId=typeof DEFENSE_UPGRADES[number]["id"];
export interface DefenseEnemy {id:number;kind:EnemyKind;x:number;y:number;hp:number;maxHP:number;speed:number;damage:number;shield:number;cooldown:number;burn:number;burnTime:number;slow:number;freeze:number}
export interface DefenseState {version:2;seed:number;phase:"ready"|"playing"|"choice"|"won"|"lost";wave:number;mode:"campaign"|"endless";hp:number;shield:number;coins:number;score:number;kills:number;upgrades:Record<UpgradeId,number>;cards:Record<string,number>;offers:string[];pendingUpgrades:number;choiceResume?:"playing"|"won";enemies:DefenseEnemy[];spawnLeft:number;spawnTime:number;shotTime:number;time:number;waveTime:number;nextId:number;traces:{x:number;y:number;life:number;color:string}[];perk:"power"|"guard"|"wealth";settled:boolean;last:string}
export interface DefenseMeta {version:1;bestWave:number;wins:number;shards:number;talents:{power:number;guard:number;wealth:number}}
export function createDefenseMeta():DefenseMeta{return{version:1,bestWave:0,wins:0,shards:0,talents:{power:0,guard:0,wealth:0}};}
export function validDefenseMeta(v:unknown):v is DefenseMeta{return object(v)&&v.version===1&&[v.bestWave,v.wins,v.shards].every(n=>integer(n,0,Number.MAX_SAFE_INTEGER))&&object(v.talents)&&[v.talents.power,v.talents.guard,v.talents.wealth].every(n=>integer(n,0,5));}
export function createDefense(seed=Date.now()>>>0,mode:DefenseState['mode']='campaign'):DefenseState{return{version:2,seed,phase:"ready",wave:1,mode,hp:180,shield:45,coins:45,score:0,kills:0,upgrades:{attack:0,rate:0,health:0,range:0,regen:0,income:0},cards:{},offers:[],pendingUpgrades:0,enemies:[],spawnLeft:0,spawnTime:0,shotTime:0,time:0,waveTime:0,nextId:1,traces:[],perk:"guard",settled:false,last:"先选起始天赋。强化次数逐波累计，点击强化时才暂停选卡。"};}
// 无尽没有玩法等级上限；只保留 JavaScript 可精确保存的数值边界。
const defenseValueLimit=(s:DefenseState)=>s.mode==='endless'?Number.MAX_SAFE_INTEGER:1e9;
export function defenseStats(s:DefenseState){
  const c=(id:string)=>s.cards[id]||0,u=s.upgrades;
  const maxHP=clamp(180+u.health*25+c("health")*40+(s.perk==="guard"?30:0)-c("overclock")*8,1,defenseValueLimit(s));
  const low=s.hp<maxHP/2?c("fortress"):0;
  const rate=1.8*(1+u.rate*.13+c("speed")*.16+c("overclock")*.25),range=150+u.range*8+c("range")*14;
  // 发射频率、射程与连锁目标数仍有画面上限，溢出的升级继续转化为伤害，避免空升级。
  const mastery=s.mode==='endless'?Object.values(s.cards).reduce((sum,level)=>sum+Math.max(0,level-6),0):0;
  const scaling=s.mode==='endless'?Math.max(1,rate/18)*Math.max(1,range/300)*(1+mastery*.04):1;
  return{maxHP,maxShield:Math.min(defenseValueLimit(s),45+c("shield")*35),damage:Math.min(s.mode==='endless'?defenseValueLimit(s):1e8,18*(1+u.attack*.24+c("power")*.22+c("pierce")*.08+c("growth")*.005*s.wave+low*.2)*(s.perk==="power"?1.1:1)*scaling),rate:Math.min(18,rate),range:Math.min(300,range),multi:Math.min(6,1+c("multi")),crit:Math.min(.75,.05+c("crit")*.09+c("fortune")*.025),armor:Math.min(.72,c("armor")*.075+low*.07),regen:1.5+u.regen*.6+c("regen")*.8,shieldRegen:c("barrier")*1.4,coinBoost:1+u.income*.1+c("income")*.15};
}
export function defenseUpgradeCost(s:DefenseState,id:UpgradeId){
  const level=s.upgrades[id];
  // 前20级沿用原曲线，随后改为平方增长，避免无尽后期价格指数膨胀而无法继续升级。
  const price=s.mode==='endless'&&level>20?12*1.28**20*(level/20)**2:12*1.28**level;
  return Math.max(8,Math.round(Math.min(defenseValueLimit(s),price)*(1-Math.min(.4,(s.cards.discount||0)*.07))));
}
export function upgradeDefense(s:DefenseState,id:UpgradeId){if(!["ready","playing","choice"].includes(s.phase)||!DEFENSE_UPGRADES.some(x=>x.id===id))return false;const cost=defenseUpgradeCost(s,id);if(s.coins<cost||(s.mode==='campaign'&&s.upgrades[id]>=100)||!Number.isSafeInteger(s.upgrades[id]+1))return false;s.coins-=cost;s.upgrades[id]++;if(id==="health")s.hp=Math.min(defenseStats(s).maxHP,s.hp+25);return true;}
export function startDefenseWave(s:DefenseState){
  s.phase="playing";s.spawnLeft=Math.min(36,5+Math.ceil(s.wave*1.2));s.spawnTime=.2;s.waveTime=0;s.enemies=[];
  s.last=s.wave%10===0?`首领波 ${s.wave}：先检查塔身和攻击升级。`:`第 ${s.wave} 波来袭`;
}
export function startDefense(s:DefenseState,meta:DefenseMeta){if(s.phase!=="ready")return;const level=meta.talents[s.perk];if(s.perk==="power")s.upgrades.attack+=level;if(s.perk==="guard")s.upgrades.health+=level;if(s.perk==="wealth")s.coins+=25+level*8;s.hp=defenseStats(s).maxHP;startDefenseWave(s);}
/** 当前战况直接转为无尽；只有已通关的30波才推进到31波，不重置血量、升级或选卡。 */
export function enterDefenseEndless(s:DefenseState){
  if(s.mode==='endless'||s.phase==='lost')return false;
  s.mode='endless';
  if(s.phase==='won'){s.wave++;startDefenseWave(s);}
  else if(s.phase==='choice'&&s.choiceResume==='won'){s.wave++;s.choiceResume='playing';s.spawnLeft=Math.min(36,5+Math.ceil(s.wave*1.2));s.spawnTime=.2;s.waveTime=0;s.enemies=[];}
  return true;
}
function spawnEnemy(s:DefenseState,kind?:EnemyKind,x?:number,y?:number){
  const pool:EnemyKind[]=["normal","swarm",...(s.wave>=3?["runner" as const]:[]),...(s.wave>=5?["armored" as const]:[]),...(s.wave>=8?["shielded" as const]:[]),...(s.wave>=12?["splitter" as const,"shooter" as const]:[]),...(s.wave>=16?["healer" as const]:[])];
  kind=kind||pool[Math.floor(random(s)*pool.length)]!;const a=random(s)*Math.PI*2;
  // 第6波起平滑增加压力，第25波后额外生命加成维持20%，避免突然跳难度。
  const pressure=1+Math.min(.2,Math.max(0,s.wave-5)*.01);
  const base=Math.min(s.mode==='endless'?defenseValueLimit(s)/8:1e8,(21+s.wave*2.6+s.wave*s.wave*.27)*pressure);
  const hp=base*(kind==="boss"?8:kind==="swarm"?.5:kind==="runner"?.7:kind==="armored"?1.4:1);
  if(s.enemies.length>=65)return;
  s.enemies.push({id:s.nextId++,kind,x:x??300+Math.cos(a)*280,y:y??200+Math.sin(a)*180,hp,maxHP:hp,speed:(kind==="boss"?6.5:kind==="runner"?21:kind==="swarm"?16:kind==="armored"?7:10)*(1.06+Math.min(.14,s.wave*.004)),damage:kind==="boss"?17+s.wave*.28:4+s.wave*.14,shield:kind==="shielded"?base*.35:0,cooldown:0,burn:0,burnTime:0,slow:0,freeze:0});
}
export function defenseOffers(s:DefenseState,refresh=false){
  if(s.pendingUpgrades<1||!['playing','won','choice'].includes(s.phase))return false;
  if(s.phase!=='choice'){s.choiceResume=s.phase as 'playing'|'won';s.phase='choice';}
  const dependencies:Record<string,string[]>={radius:['blast'],links:['chain'],inferno:['burn'],freeze:['slow'],storm:['chain','freeze']};
  const available=(id:string)=>s.mode==='endless'?Number.isSafeInteger((s.cards[id]||0)+1):(s.cards[id]||0)<6;
  const pool=DEFENSE_CARDS.filter(c=>available(c.id)&&(!dependencies[c.id]||dependencies[c.id]!.every(id=>s.cards[id])));
  if(!refresh&&s.offers.length&&s.offers.every(id=>pool.some(card=>card.id===id)))return true;
  const offers:string[]=[],previous=new Set(s.offers);
  if(!refresh&&s.wave<=5){const starter=["power","speed","multi","blast"][Math.max(0,s.wave-2)]!;if(starter&&available(starter))offers.push(starter);}
  // 洗牌抽取，优先给出上一组以外的牌；刷新不消耗金币或强化次数。
  const shuffled=[...pool];for(let i=shuffled.length-1;i>0;i--){const j=Math.floor(random(s)*(i+1));[shuffled[i],shuffled[j]]=[shuffled[j]!,shuffled[i]!];}
  shuffled.sort((a,b)=>Number(previous.has(a.id))-Number(previous.has(b.id)));
  for(const card of shuffled)if(offers.length<3&&!offers.includes(card.id))offers.push(card.id);
  if(!pool.length){s.coins=Math.min(defenseValueLimit(s),s.coins+100);s.hp=defenseStats(s).maxHP;s.pendingUpgrades--;s.offers=[];closeDefenseOffers(s);s.last='强化已全部满级，本次兑换100金币并修复塔身。';return false;}
  s.offers=offers;return true;
}
export function closeDefenseOffers(s:DefenseState){if(s.phase!=='choice')return false;s.phase=s.choiceResume||'playing';delete s.choiceResume;return true;}
export function pickDefenseCard(s:DefenseState,id:string){
  if(s.phase!=="choice"||s.pendingUpgrades<1||!s.offers.includes(id)||(s.mode==='campaign'&&(s.cards[id]||0)>=6)||!Number.isSafeInteger((s.cards[id]||0)+1))return false;
  s.cards[id]=(s.cards[id]||0)+1;
  const stats=defenseStats(s);if(id==="health")s.hp=Math.min(stats.maxHP,s.hp+40);if(id==="shield")s.shield=stats.maxShield;
  s.pendingUpgrades--;s.offers=[];closeDefenseOffers(s);s.last=`已获得强化，剩余 ${s.pendingUpgrades} 次可随时选择。`;return true;
}
function incoming(s:DefenseState,damage:number){const stats=defenseStats(s);damage*=1-stats.armor;const shield=Math.min(s.shield,damage);s.shield-=shield;s.hp=Math.max(0,s.hp-(damage-shield));}
function strike(s:DefenseState,e:DefenseEnemy,damage:number,critical=false){
  if(e.hp<=0)return;
  damage*=e.kind==="armored"?Math.min(1,.58+(s.cards.pierce||0)*.1):1;
  if(s.cards.execute&&e.hp/e.maxHP<.3)damage*=1+(s.cards.execute||0)*.2;
  if(s.cards.focus&&e.kind==="boss")damage*=1+(s.cards.focus||0)*.25;
  const absorbed=Math.min(e.shield,damage);e.shield-=absorbed;e.hp-=damage-absorbed;
  if(critical&&s.cards.fortune)s.coins=Math.min(defenseValueLimit(s),s.coins+(s.cards.fortune||0));
  if(s.cards.leech)s.hp=Math.min(defenseStats(s).maxHP,s.hp+Math.min(8,damage*(s.cards.leech||0)*.015));
}
function fireDefense(s:DefenseState){
  const stats=defenseStats(s),targets=s.enemies.filter(e=>e.hp>0&&Math.hypot(e.x-300,e.y-200)<=stats.range).sort((a,b)=>((s.cards.focus&&a.kind==="boss")?-1000:Math.hypot(a.x-300,a.y-200))-((s.cards.focus&&b.kind==="boss")?-1000:Math.hypot(b.x-300,b.y-200)));
  for(const e of targets.slice(0,stats.multi)){
    const crit=random(s)<stats.crit,damage=stats.damage*(crit?2:1);strike(s,e,damage,crit);
    s.traces.push({x:e.x,y:e.y,life:.16,color:crit?"#f1ca75":"#77c6fa"});
    if(s.cards.burn){e.burn=Math.min(defenseValueLimit(s),stats.damage*.13*s.cards.burn);e.burnTime=Math.min(defenseValueLimit(s),3+(s.cards.inferno||0));}
    if(s.cards.slow)e.slow=2.5;
    if(s.cards.freeze&&random(s)<Math.min(.55,.12*s.cards.freeze))e.freeze=e.kind==="boss"?.4:1.1;
    if(s.cards.blast)for(const near of s.enemies)if(near!==e&&Math.hypot(near.x-e.x,near.y-e.y)<34+(s.cards.radius||0)*9)strike(s,near,damage*(.3+.1*s.cards.blast+.04*(s.cards.radius||0)));
    if(s.cards.chain){const chain=s.enemies.filter(near=>near!==e&&near.hp>0&&Math.hypot(near.x-e.x,near.y-e.y)<90+(s.cards.links||0)*15).slice(0,Math.min(8,1+s.cards.chain+(s.cards.links||0)));for(const near of chain){strike(s,near,damage*.5*(near.freeze>0&&s.cards.storm?1+s.cards.storm*.4:1));s.traces.push({x:near.x,y:near.y,life:.2,color:"#c2a5ff"});}}
  }
}
export function settleDefense(s:DefenseState,meta:DefenseMeta){
  meta.bestWave=Math.max(meta.bestWave,s.phase==="won"||s.choiceResume==='won'?s.wave:s.wave-1);
  if(!s.settled&&(s.phase==="won"||s.phase==="lost")){meta.shards=Math.min(Number.MAX_SAFE_INTEGER,meta.shards+Math.floor((s.wave-1)/3));if(s.phase==="won"){meta.wins=Math.min(Number.MAX_SAFE_INTEGER,meta.wins+1);meta.shards=Math.min(Number.MAX_SAFE_INTEGER,meta.shards+5);}s.settled=true;}
}
export function stepDefense(s:DefenseState,dt:number){
  if(s.phase!=="playing"||!finite(dt,.000001,.2))return;
  s.time+=dt;s.waveTime+=dt;s.shotTime+=dt;const stats=defenseStats(s);
  s.hp=Math.min(stats.maxHP,s.hp+stats.regen*dt);s.shield=Math.min(stats.maxShield,s.shield+stats.shieldRegen*dt);
  if(s.cards.reserve&&s.hp<stats.maxHP*.3&&Math.floor(s.time/8)!==Math.floor((s.time-dt)/8))s.shield=Math.min(stats.maxShield,s.shield+15*s.cards.reserve);
  // 满屏时保留待入场敌人，等有空位再出怪，不丢弃波次，也不增加同屏负担。
  if(s.spawnLeft>0){s.spawnTime=Math.max(0,s.spawnTime-dt);if(s.spawnTime<=0&&s.enemies.length<65){const boss=s.wave%10===0&&s.spawnLeft===Math.min(36,5+Math.ceil(s.wave*1.2));spawnEnemy(s,boss?"boss":undefined);s.spawnLeft--;s.spawnTime=Math.max(.3,.76-s.wave*.01);}}
  if(s.shotTime>=1/stats.rate){s.shotTime=0;fireDefense(s);}
  if(s.cards.nova&&Math.floor(s.time/5)!==Math.floor((s.time-dt)/5))for(const e of s.enemies)if(Math.hypot(e.x-300,e.y-200)<115){strike(s,e,stats.damage*(1+s.cards.nova*.5));const a=Math.atan2(e.y-200,e.x-300);e.x+=Math.cos(a)*18;e.y+=Math.sin(a)*18;}
  for(const e of [...s.enemies]){
    if(e.hp<=0)continue;
    e.cooldown-=dt;e.slow=Math.max(0,e.slow-dt);e.freeze=Math.max(0,e.freeze-dt);
    if(e.burnTime>0){e.hp-=e.burn*dt;e.burnTime=Math.max(0,e.burnTime-dt);}if(e.hp<=0)continue;
    // 远射者在塔的基础射程外施压，升级射程或选远程瞄具可以应对。
    const distance=Math.hypot(e.x-300,e.y-200),reach=e.kind==="shooter"?185+Math.min(40,Math.max(0,s.wave-12)*2):24;
    if(e.freeze<=0&&distance>reach){const speed=e.speed*(e.slow>0?Math.max(.35,1-(s.cards.slow||0)*.12):1);e.x+=(300-e.x)/distance*speed*dt;e.y+=(200-e.y)/distance*speed*dt;}
    else if(e.freeze<=0&&e.cooldown<=0){incoming(s,e.damage);if(s.cards.thorns&&distance<35)strike(s,e,e.damage*s.cards.thorns*.8);e.cooldown=e.kind==="shooter"?2.5:1.3;}
    if(e.kind==="healer"&&e.cooldown<=0){for(const near of s.enemies)if(Math.hypot(near.x-e.x,near.y-e.y)<65)near.hp=Math.min(near.maxHP,near.hp+near.maxHP*.05);e.cooldown=3;}
  }
  const dead=s.enemies.filter(e=>e.hp<=0);s.enemies=s.enemies.filter(e=>e.hp>0);
  for(const e of dead){s.kills++;const gain=Math.round((3+s.wave*.25)*stats.coinBoost*(e.kind==="boss"?8+(s.cards.bounty||0)*2:e.kind==="armored"||e.kind==="shielded"?1+(s.cards.bounty||0)*.25:1));s.coins=Math.min(defenseValueLimit(s),s.coins+gain);s.score=Math.min(defenseValueLimit(s),s.score+10+s.wave*2);if(s.cards.healKill)s.hp=Math.min(stats.maxHP,s.hp+1.4*s.cards.healKill);
    if(e.kind==="splitter"){spawnEnemy(s,"swarm",e.x-8,e.y);spawnEnemy(s,"swarm",e.x+8,e.y);}
    if(e.burnTime>0&&s.cards.inferno)for(const near of s.enemies)if(Math.hypot(near.x-e.x,near.y-e.y)<60){near.burn=e.burn;near.burnTime=3;}
  }
  s.traces=s.traces.map(t=>({...t,life:t.life-dt})).filter(t=>t.life>0).slice(-70);
  if(s.hp<=0){s.phase="lost";s.last="这次走得很远了。带上新的天赋，再试一种组合。";return;}
  if(s.spawnLeft===0&&s.enemies.length===0){
    s.hp=Math.min(stats.maxHP,s.hp+stats.maxHP*.07+(s.cards.supply||0)*8);s.shield=Math.min(stats.maxShield,s.shield+8);
    s.coins=Math.min(defenseValueLimit(s),s.coins+12+s.wave*2+(s.cards.supply||0)*5+Math.min(75,Math.floor(s.coins*.025*(s.cards.interest||0))));
    s.pendingUpgrades=Math.min(Number.MAX_SAFE_INTEGER,s.pendingUpgrades+1);
    if(s.wave===30&&s.mode==="campaign"){s.phase="won";s.score+=1000;s.last="三十波守住了！可以继续挑战无尽。";}else{const cleared=s.wave;s.wave++;startDefenseWave(s);s.last=`第 ${cleared} 波已守住，强化已累计 ${s.pendingUpgrades} 次。`;}
  }
}
export function validDefense(v:unknown):v is DefenseState {
  if(!object(v)||v.version!==2||!integer(v.pendingUpgrades,0,Number.MAX_SAFE_INTEGER)||(v.choiceResume!==undefined&&!['playing','won'].includes(v.choiceResume as string))||(v.phase==='choice'&&(Number(v.pendingUpgrades)<1||v.choiceResume===undefined))||!integer(v.seed,0,4294967295)||!integer(v.wave,1,v.mode==='endless'?Number.MAX_SAFE_INTEGER:100000)||!["ready","playing","choice","won","lost"].includes(v.phase as string)||!["campaign","endless"].includes(v.mode as string)||!["power","guard","wealth"].includes(v.perk as string))return false;
  const limit=v.mode==='endless'?Number.MAX_SAFE_INTEGER:1e12;
  if(![v.hp,v.shield,v.coins,v.score,v.kills,v.spawnLeft,v.spawnTime,v.shotTime,v.time,v.waveTime,v.nextId].every(n=>finite(n,-1,limit))||typeof v.settled!=="boolean"||typeof v.last!=="string"||!object(v.upgrades)||!DEFENSE_UPGRADES.every(u=>integer((v.upgrades as Record<string,unknown>)[u.id],0,v.mode==='endless'?Number.MAX_SAFE_INTEGER:100))||!object(v.cards)||!Object.entries(v.cards).every(([id,n])=>DEFENSE_CARDS.some(c=>c.id===id)&&integer(n,0,v.mode==='endless'?Number.MAX_SAFE_INTEGER:6)))return false;
  if(!Array.isArray(v.offers)||v.offers.length>3||new Set(v.offers).size!==v.offers.length||!v.offers.every(id=>DEFENSE_CARDS.some(c=>c.id===id))||!Array.isArray(v.enemies)||v.enemies.length>65)return false;
  if(!v.enemies.every(e=>object(e)&&integer(e.id,0,v.mode==='endless'?Number.MAX_SAFE_INTEGER:1e9)&&Object.hasOwn(DEFENSE_ENEMY_NAMES,String(e.kind))&&[e.x,e.y,e.hp,e.maxHP,e.speed,e.damage,e.shield,e.cooldown,e.burn,e.burnTime,e.slow,e.freeze].every(n=>finite(n,-1e9,v.mode==='endless'?Number.MAX_SAFE_INTEGER:1e9))))return false;
  return Array.isArray(v.traces)&&v.traces.length<=70&&v.traces.every(t=>object(t)&&finite(t.x,-1000,1000)&&finite(t.y,-1000,1000)&&finite(t.life,0,1)&&typeof t.color==="string");
}
/** 旧版被强制停在波末的选卡改为待领一次，战斗从下一波继续，原抽牌和全部数值保留。 */
export function migrateDefense(v:unknown):DefenseState|null{
  if(!object(v)||v.version!==1)return null;
  const next={...v,version:2,pendingUpgrades:v.phase==='choice'?1:0,choiceResume:v.phase==='choice'?'playing':undefined} as DefenseState;
  if(!validDefense(next))return null;
  if(next.phase==='choice'){const offers=next.offers;next.wave++;startDefenseWave(next);next.offers=offers;delete next.choiceResume;}
  return next;
}
