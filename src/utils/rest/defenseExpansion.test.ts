// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { configureDefense, createDefense, createDefenseMeta, DEFENSE_CORES, DEFENSE_ENEMY_LIMIT, DEFENSE_EVOLUTIONS, defenseChoiceCatalog, defenseFeatures, defenseOffers, defensePurchaseQuote, defenseRegion, defenseStats, defenseUpgradeCost, pickDefenseCard, purchaseDefenseLevels, selectDefenseCore, startDefense, startDefenseWave, stepDefense, validDefense, type DefenseEnemy, type DefenseState } from './defense';

const enemy=(id=1,hp=1e10):DefenseEnemy=>({id,kind:'normal',x:345+id,y:200,hp,maxHP:hp,speed:0,damage:1,shield:0,cooldown:1,burn:0,burnTime:0,slow:0,freeze:0});
function arena(){const s=createDefense(7,'endless');startDefense(s,createDefenseMeta());s.enemies=[enemy()];s.nextId=2;s.spawnLeft=1;s.spawnTime=100;return s;}

describe('塔芯、搜索与连续强化',()=>{
  it('四种塔芯有不同实际属性，开局后不能来回切换刷补血',()=>{
    const stats=DEFENSE_CORES.map(core=>{const s=createDefense(11);expect(selectDefenseCore(s,core.id)).toBe(true);startDefense(s,createDefenseMeta());expect(selectDefenseCore(s,'rapid')).toBe(false);return defenseStats(s);});
    expect(stats[0]!.rate).toBeGreaterThan(stats[1]!.rate);expect(stats[1]!.damage).toBeGreaterThan(stats[0]!.damage);
    expect(stats[1]!.blast).toBeGreaterThan(0);expect(stats[2]!.chain).toBe(1);expect(stats[3]!.maxHP).toBeGreaterThan(stats[0]!.maxHP);
  });
  it('v2旧存档原样可读，接入时保留模式、金币、强化、波数及现场，续存可确定复现',()=>{
    const s=arena();s.wave=51;s.coins=4321;s.pendingUpgrades=7;s.cards={speed:8,range:12};delete s.features;
    const raw=JSON.stringify(s);expect(validDefense(s)).toBe(true);expect(JSON.stringify(s)).toBe(raw);
    const restored=JSON.parse(raw) as DefenseState;
    expect(selectDefenseCore(restored,'tesla')).toBe(true);expect(restored.mode).toBe('endless');expect(restored.wave).toBe(51);
    expect(restored.coins).toBe(4321);expect(restored.cards).toEqual(s.cards);expect(restored.enemies).toEqual(s.enemies);expect(restored.pendingUpgrades).toBe(7);
    const copy=JSON.parse(JSON.stringify(restored));for(let i=0;i<50;i++){stepDefense(restored,.05);stepDefense(copy,.05);}
    expect(copy).toEqual(restored);expect(validDefense(copy)).toBe(true);expect(defenseFeatures(restored).log.startWave).toBe(51);
  });
  it('搜索整个技能库，前置条件仍生效；剩余强化连续选择，不推进波数和战斗时间',()=>{
    const s=arena();s.pendingUpgrades=3;defenseOffers(s);const before={wave:s.wave,time:s.time,coins:s.coins,enemies:structuredClone(s.enemies)};
    expect(defenseChoiceCatalog(s,'吸血').map(c=>c.id)).toContain('leech');expect(defenseChoiceCatalog(s,'火').length).toBeGreaterThan(1);
    expect(defenseChoiceCatalog(s,'碎冰')[0]!.requirements.length).toBeGreaterThan(0);expect(pickDefenseCard(s,'evo:froststorm',true)).toBe(false);
    expect(pickDefenseCard(s,'leech',true)).toBe(true);expect(s.phase).toBe('choice');expect(s.pendingUpgrades).toBe(2);
    expect(pickDefenseCard(s,'leech',true)).toBe(true);expect(s.phase).toBe('choice');
    expect(pickDefenseCard(s,'leech',true)).toBe(true);expect(s.phase).toBe('playing');expect(s.cards.leech).toBe(3);
    expect({wave:s.wave,time:s.time,coins:s.coins,enemies:s.enemies}).toEqual(before);expect(pickDefenseCard(s,'leech',true)).toBe(false);
  });
  it('免费刷新和偏好不扣强化次数，配方达到后可以选择并继续提升进化',()=>{
    const s=arena();s.pendingUpgrades=4;configureDefense(s,{preference:'爆破'});defenseOffers(s);const coins=s.coins;
    for(let i=0;i<30;i++){defenseOffers(s,true);expect(s.offers).toHaveLength(3);expect(s.offers.some(id=>defenseChoiceCatalog(s).find(c=>c.id===id)?.family==='爆破')).toBe(true);}
    expect(s.coins).toBe(coins);expect(s.pendingUpgrades).toBe(4);
    for(const evo of DEFENSE_EVOLUTIONS){for(const [id,level] of Object.entries(evo.requirements))s.cards[id]=level;expect(pickDefenseCard(s,'evo:'+evo.id,true)).toBe(true);}
    expect(Object.values(defenseFeatures(s).evolutions)).toEqual([1,1,1,1]);expect(s.phase).toBe('playing');expect(validDefense(s)).toBe(true);
  });
});

describe('取消旧技能上限与完整结算',()=>{
  it('两种模式都可超过旧等级与射程、次数、目标上限，概率溢出和护甲仍有实际收益',()=>{
    for(const mode of ['campaign','endless'] as const){
      const s=createDefense(1,mode);s.coins=1e9;s.upgrades.rate=200;s.upgrades.range=200;s.upgrades.attack=100;
      s.cards={multi:30,chain:30,links:30,radius:60,crit:30,armor:50,discount:60};
      const x=defenseStats(s);expect(x.rate).toBeGreaterThan(18);expect(x.range).toBeGreaterThan(300);expect(x.multi).toBe(31);expect(x.chain).toBe(61);expect(x.chainRange).toBeGreaterThan(300);expect(x.blastRadius).toBeGreaterThan(300);
      expect(x.crit).toBe(1);expect(x.critMultiplier).toBeGreaterThan(2);expect(x.armor).toBeGreaterThan(.72);expect(x.armor).toBeLessThan(1);
      expect(purchaseDefenseLevels(s,'attack',5)).toBe(5);expect(s.upgrades.attack).toBe(105);
      const cost=defenseUpgradeCost(s,'range');s.cards.discount++;expect(defenseUpgradeCost(s,'range')).toBeLessThan(cost);expect(validDefense(s)).toBe(true);
    }
  });
  it('超过每秒18轮的攻击按完整次数命中，计算量不会随次数逐次循环',()=>{
    const s=arena();s.upgrades.rate=1000000;s.cards.crit=20;const x=defenseStats(s),hp=s.enemies[0]!.hp;
    stepDefense(s,.1);const shots=Math.floor(x.rate*.1+1e-9);
    expect(defenseFeatures(s).log.shots).toBe(shots);expect(shots).toBeGreaterThan(10000);
    expect(hp-s.enemies[0]!.hp).toBeCloseTo(shots*x.damage*x.critMultiplier,3);expect(s.traces.length).toBeLessThanOrEqual(32);expect(validDefense(s)).toBe(true);
  });
  it('旧无尽高射程升级的伤害收益保留，取消攻速截断后总体输出不会被清零',()=>{
    const s=createDefense(8,'endless');delete s.features;s.upgrades.rate=200;s.upgrades.range=200;
    const rawRate=1.8*(1+200*.13),rawRange=150+200*8;
    const oldDamage=18*Math.max(1,rawRate/18)*Math.max(1,rawRange/300);
    const x=defenseStats(s);expect(x.range).toBe(rawRange);expect(x.rate).toBe(rawRate);
    expect(x.damage*x.rate).toBeCloseTo(oldDamage*Math.min(18,rawRate),6);expect(validDefense(s)).toBe(true);
  });
  it('超过6个直击目标和8个连锁目标均真正受到伤害，大范围技能能覆盖远处敌人',()=>{
    const direct=arena();direct.enemies=Array.from({length:20},(_,i)=>enemy(i+1));direct.nextId=21;direct.cards.multi=30;
    stepDefense(direct,.2);stepDefense(direct,.2);stepDefense(direct,.1);expect(direct.enemies.filter(e=>e.hp<e.maxHP)).toHaveLength(20);
    const chain=arena();chain.enemies=Array.from({length:20},(_,i)=>enemy(i+1));chain.nextId=21;chain.cards={chain:30,links:30};
    stepDefense(chain,.2);stepDefense(chain,.2);stepDefense(chain,.1);expect(chain.enemies.filter(e=>e.hp<e.maxHP)).toHaveLength(20);
    const far=arena();far.enemies=[enemy(1),{...enemy(2),x:650}];far.cards={blast:1,radius:100};
    stepDefense(far,.2);stepDefense(far,.2);stepDefense(far,.1);expect(far.enemies[1]!.hp).toBeLessThan(far.enemies[1]!.maxHP);
  });
  it('四种进化都有真实伤害记录，而非只有名称和特效',()=>{
    for(const id of ['firestorm','froststorm','bulwark','volley'] as const){
      const s=arena(),f=defenseFeatures(s);f.evolutions[id]=1;s.enemies=[enemy(1),enemy(2),enemy(3)];s.nextId=4;
      if(id==='firestorm'){s.cards.burn=3;s.enemies[0]!.hp=1;}
      if(id==='froststorm'){s.cards.chain=3;s.enemies[1]!.freeze=2;}
      if(id==='bulwark'){s.shield=1;s.enemies[0]!.x=310;s.enemies[0]!.cooldown=0;s.enemies[0]!.damage=50;}
      if(id==='volley')f.log.shots=11;
      for(let i=0;i<8;i++)stepDefense(s,.1);
      expect(f.log.damage.evolution,id).toBeGreaterThan(0);expect(validDefense(s),id).toBe(true);
    }
  });
});

describe('自动购买、怪物、无尽和性能',()=>{
  it('批量报价与实际购买一致，自动购买保留金币，暂停选卡不花金币',()=>{
    const s=arena();s.coins=1200;const quote=defensePurchaseQuote(s,'attack',5),coins=s.coins;
    expect(purchaseDefenseLevels(s,'attack',5)).toBe(quote.levels);expect(s.coins).toBe(coins-quote.cost);
    configureDefense(s,{auto:{enabled:true,priority:'attack',batch:5,reserve:300}});for(let i=0;i<30;i++)stepDefense(s,.05);
    expect(s.coins).toBeGreaterThanOrEqual(300);expect(s.upgrades.attack).toBeGreaterThan(5);
    s.pendingUpgrades=2;defenseOffers(s);const before=JSON.stringify(s);for(let i=0;i<30;i++)stepDefense(s,.05);expect(JSON.stringify(s)).toBe(before);
    configureDefense(s,{auto:{reserve:-30}});expect(defenseFeatures(s).auto.reserve).toBe(0);
  });
  it('每10波首领轮换，区域每20波变化，高波数仍持续生成且可保存',()=>{
    expect([10,20,30].map(wave=>{const s=createDefense(11,'endless');s.wave=wave;startDefenseWave(s);stepDefense(s,.2);expect(validDefense(s)).toBe(true);return s.enemies[0]!.boss;})).toEqual(['ram','bulwark','summoner']);
    const s=createDefense(1,'endless');const regions=[1,21,41,61].map(wave=>{s.wave=wave;return defenseRegion(s).id;});expect(new Set(regions).size).toBe(4);
    s.wave=100001;startDefenseWave(s);stepDefense(s,.2);expect(s.spawnLeft).toBeGreaterThan(36);expect(validDefense(JSON.parse(JSON.stringify(s)))).toBe(true);
  });
  it('死亡分裂援军保留队列，同屏不会超过65，统计不重复计算过量伤害',()=>{
    const s=arena();s.enemies=Array.from({length:65},(_,i)=>({...enemy(i+1,1000),x:560}));s.nextId=66;s.spawnTime=0;s.enemies[0]!.kind='splitter';s.enemies[0]!.hp=0;
    stepDefense(s,.05);expect(defenseFeatures(s).reinforcements[0]!.count).toBe(2);
    for(let i=0;i<20;i++)stepDefense(s,.05);expect(s.enemies.length).toBeLessThanOrEqual(65);expect(defenseFeatures(s).reinforcements[0]!.count).toBe(1);
    const hit=arena();hit.enemies[0]!.hp=hit.enemies[0]!.maxHP=1;hit.upgrades.attack=1000;stepDefense(hit,.2);stepDefense(hit,.2);stepDefense(hit,.1);
    expect(defenseFeatures(hit).log.damage.direct).toBe(1);
  });
  it('远程敌人会逐步前压，低射程高回复的旧局也能结束当前波',()=>{
    const s=arena();s.spawnLeft=0;s.enemies=[{...enemy(1,1),kind:'shooter',x:300+Math.cos(.265)*270,y:200+Math.sin(.265)*270,speed:10,damage:2,cooldown:0}];s.upgrades.regen=20;
    for(let i=0;i<1200&&s.wave===1;i++)stepDefense(s,.05);
    expect(s.wave).toBe(2);expect(s.hp).toBeGreaterThan(0);expect(s.phase).toBe('playing');expect(validDefense(s)).toBe(true);
  });
  it('高等级攻速、全屏爆破与碎冰连锁并用时仍按固定目标预算计算',()=>{
    const s=arena();s.enemies=Array.from({length:DEFENSE_ENEMY_LIMIT},(_,i)=>({...enemy(i+1,1e15),x:320+i%10*3,y:170+Math.floor(i/10)*4,freeze:10}));s.nextId=66;
    s.upgrades.rate=2000;s.cards={multi:100,chain:100,links:100,blast:100,radius:100,slow:100,freeze:100,storm:100};defenseFeatures(s).evolutions.froststorm=1;
    const started=performance.now();for(let i=0;i<250;i++)stepDefense(s,.05);const elapsed=performance.now()-started;
    console.info('塔防65目标/高等级组合：250步耗时',Math.round(elapsed),'ms');
    expect(s.enemies).toHaveLength(65);expect(elapsed).toBeLessThan(2000);expect(s.traces.length).toBeLessThanOrEqual(32);
    expect(JSON.stringify(s).length).toBeLessThan(50000);expect(validDefense(JSON.parse(JSON.stringify(s)))).toBe(true);
  });
});
