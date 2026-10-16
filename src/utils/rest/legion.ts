import { LEGION_CARDS, LEGION_ENEMIES, LEGION_REGIONS, LEGION_STRATEGIES, LEGION_UNITS, type LegionCardId, type LegionEnemyKind, type LegionStrategy, type LegionUnitKind } from './legionContent';

export const LEGION_ACTOR_LIMIT = 56;
export const LEGION_EFFECT_LIMIT = 28;
const limit = (value: number, max = 1e12) => Math.max(0, Math.min(max, value));
// replenish 仅保留旧存档字段，当前关卡不会补回阵亡的兵。
type Options = { summon: boolean; strengthen: boolean; replenish: boolean; retry: boolean; train: boolean };
export interface LegionActor {
  id: number; kind: LegionUnitKind | LegionEnemyKind; side: 'ally' | 'enemy'; x: number; y: number;
  hp: number; maxHP: number; cooldown: number; attack: number; slow: number; burn: number; burnTime: number; skill: number;
}
export interface LegionEffect { id: number; kind: 'arrow' | 'fire' | 'lightning' | 'heal' | 'slash' | 'hit'; x: number; y: number; tx: number; ty: number; life: number; text: string; source?: LegionActor['kind'] }
export interface LegionState {
  version: 1; seed: number; nextId: number; wave: number; bestWave: number; cleared: number; elapsed: number;
  phase: 'battle' | 'march' | 'recover'; phaseTime: number; waveTime: number; paused: boolean; speed: 1 | 2;
  strategy: LegionStrategy; options: Options; coins: number; essence: number; xp: number; rank: number;
  training: Record<LegionUnitKind, number>; formation: Record<LegionUnitKind, number>; cards: Partial<Record<LegionCardId, number>>; pending: number;
  allies: LegionActor[]; enemies: LegionActor[]; spawnLeft: number; spawnClock: number; recruitClock: number; trainClock: number;
  effects: LegionEffect[]; log: string[]; kills: number; defeats: number; wins: number; bestRun: number; damage: number; healing: number;
}
const unit = (kind: LegionUnitKind) => LEGION_UNITS.find(item => item.id === kind)!;
export const legionCardLevel = (s: LegionState, id: LegionCardId) => s.cards[id] ?? 0;
function random(s: LegionState) { s.seed = (Math.imul(s.seed, 1664525) + 1013904223) >>> 0; return s.seed / 4294967296; }
export const legionRegion = (s: LegionState) => LEGION_REGIONS[Math.floor((s.wave - 1) / 10) % LEGION_REGIONS.length]!;
export const legionChapter = (s: LegionState) => `${Math.floor((s.wave - 1) / 10) + 1}-${(s.wave - 1) % 10 + 1}`;
export const legionUnlocked = (s: LegionState, kind: LegionUnitKind) => s.bestWave >= unit(kind).unlock;
export function legionStats(s: LegionState, kind: LegionUnitKind) {
  const u = unit(kind), level = s.training[kind], c = (id: LegionCardId) => legionCardLevel(s, id);
  return { hp: u.hp * (1 + level * .1) * (1 + c('vitality') * .15),
    damage: u.damage * (1 + level * .09) * (1 + c('power') * .12) * (kind === 'archer' ? 1 + c('precision') * .18 : 1),
    rate: Math.min(12,u.rate * (1 + c('haste') * .1)), range: u.range + (kind === 'archer' ? c('precision') * 12 : 0),
    armor: u.armor / Math.max(.1, 1 - u.armor) + c('armor') * .12,
  };
}
export function legionPower(s: LegionState) { return Math.round(LEGION_UNITS.filter(u => legionUnlocked(s, u.id)).reduce((total, u) => { const st = legionStats(s, u.id); return total + st.hp + st.damage * st.rate * 10; }, 0)); }
export const legionTrainingCost = (s: LegionState, kind: LegionUnitKind) => Math.ceil(24 * Math.pow(1.17, Math.min(300, s.training[kind])));
function note(s: LegionState, message: string) { s.log.unshift(message); s.log.length = Math.min(s.log.length, 18); }
function effect(s: LegionState, kind: LegionEffect['kind'], a: LegionActor, b: LegionActor, text = '') {
  if (s.effects.length >= LEGION_EFFECT_LIMIT) return;
  const projectile=['arrow','fire','lightning','heal'].includes(kind), facing=a.side==='ally'?1:-1;
  s.effects.push({ id: s.nextId++, kind, x: a.x+(projectile?20*facing:0), y: a.y-(projectile?36:20), tx: b.x, ty: b.y - 20, life: kind === 'hit' ? .7 : .32, text, source: a.kind });
}
function addAlly(s: LegionState, kind: LegionUnitKind, newSlot = true) {
  if (s.allies.length >= 24) return;
  const st = legionStats(s, kind), index = s.allies.length;
  s.allies.push({ id: s.nextId++, kind, side: 'ally', x: 150 - Math.floor(index / 3) * 28, y: 230 + index % 3 * 58,
    hp: st.hp, maxHP: st.hp, cooldown: random(s) * .5, attack: 0, slow: 0, burn: 0, burnTime: 0, skill: 0 });
  // 记录本轮编队已入场名额，阵亡不能重新使用同一个名额绕过关后复活规则。
  if(newSlot)s.formation[kind]++;
}
const bosses: LegionEnemyKind[] = ['warlord', 'behemoth', 'necromancer'];
function addEnemy(s: LegionState, kind: LegionEnemyKind, x = 875, y = 230 + Math.floor(random(s) * 3) * 58) {
  if (s.allies.length + s.enemies.length >= LEGION_ACTOR_LIMIT) return false;
  const base = LEGION_ENEMIES[kind], boss = bosses.includes(kind);
  const scaling = (1 + (s.wave - 1) * .14) * Math.pow(1.025, Math.min(1200, s.wave - 1));
  const hp = Math.min(1e12, base.hp * scaling * (s.wave % 5 === 0 && !boss ? 1.15 : 1));
  s.enemies.push({ id: s.nextId++, kind, side: 'enemy', x, y, hp, maxHP: hp, cooldown: .6 + random(s), attack: 0, slow: 0, burn: 0, burnTime: 0, skill: boss ? 5 : 0 }); return true;
}
function beginWave(s: LegionState) {
  s.phase = 'battle'; s.phaseTime = 0; s.waveTime = 0; s.enemies = []; s.effects = [];
  s.spawnLeft = Math.min(28, 4 + Math.floor(s.wave * .8)); s.spawnClock = .2;
  for(const u of LEGION_UNITS) {
    const missing=s.formation[u.id]-s.allies.filter(a=>a.kind===u.id&&a.hp>0).length;
    for(let n=0;n<missing;n++)addAlly(s,u.id,false);
  }
  for (const a of s.allies) { a.x = 100 + random(s) * 70; a.y = 230 + (a.id % 3) * 58; a.hp = Math.min(a.maxHP, a.hp + a.maxHP * .3); }
  if (s.wave % 10 === 0) { const boss = bosses[(Math.floor(s.wave / 10) - 1) % 3]!; addEnemy(s, boss); note(s, `首领出现：${LEGION_ENEMIES[boss].name}`); }
  else note(s, `${legionRegion(s).name} ${legionChapter(s)} · ${s.wave % 5 === 0 ? '精英遭遇' : '自动推进'}`);
}
export function createLegion(seed = Date.now() >>> 0): LegionState {
  const s: LegionState = { version: 1, seed, nextId: 1, wave: 1, bestWave: 1, cleared: 0, elapsed: 0, phase: 'battle', phaseTime: 0, waveTime: 0, paused: false, speed: 1,
    strategy: 'balanced', options: { summon: true, strengthen: true, replenish: true, retry: true, train: true }, coins: 0, essence: 45, xp: 0, rank: 1,
    training: { guard: 0, sword: 0, archer: 0, mage: 0, priest: 0, giant: 0 }, formation: { guard: 0, sword: 0, archer: 0, mage: 0, priest: 0, giant: 0 }, cards: {}, pending: 0,
    allies: [], enemies: [], spawnLeft: 0, spawnClock: 0, recruitClock: 0, trainClock: 0, effects: [], log: [], kills: 0, defeats: 0, wins: 0, bestRun: 1, damage: 0, healing: 0 };
  for (const kind of ['guard', 'sword', 'archer', 'sword', 'guard', 'archer'] as const) addAlly(s, kind);
  beginWave(s); return s;
}
export function trainLegion(s: LegionState, kind: LegionUnitKind) {
  if (!legionUnlocked(s, kind) || s.training[kind] >= 300 || s.coins < legionTrainingCost(s, kind)) return false;
  s.coins -= legionTrainingCost(s, kind); s.training[kind]++;
  refreshHealth(s); return true;
}
function refreshHealth(s: LegionState) {
  for (const a of s.allies) { const hp = legionStats(s, a.kind as LegionUnitKind).hp; a.hp = Math.min(hp, a.hp + Math.max(0, hp - a.maxHP)); a.maxHP = hp; }
}
export function legionOffers(s: LegionState) {
  // 只提供已解锁兵种能使用的强化，避免自动选择尚无作用的能力。
  const available = LEGION_CARDS.filter(c => (c.family !== 'elements' || legionUnlocked(s, 'mage')) && (c.id !== 'heal' || legionUnlocked(s, 'priest')) && (c.id !== 'stomp' || legionUnlocked(s, 'giant')));
  const copy = [...available]; for (let i = copy.length - 1; i > 0; i--) { const j = Math.floor(random(s) * (i + 1)); [copy[i], copy[j]] = [copy[j]!, copy[i]!]; }
  return copy.slice(0, 3);
}
export function strengthenLegion(s: LegionState, id?: LegionCardId) {
  if (!s.pending) return false;
  if (id && ((LEGION_CARDS.find(c=>c.id===id)?.family==='elements'&&!legionUnlocked(s,'mage')) || (id==='heal'&&!legionUnlocked(s,'priest')) || (id==='stomp'&&!legionUnlocked(s,'giant')))) return false;
  const card = id ? LEGION_CARDS.find(c => c.id === id) : legionOffers(s).sort((a, b) => {
    const score = (c: typeof a) => (c.family === s.strategy ? 5 : c.family === 'balanced' ? 2 : 0) + 2 / (1 + legionCardLevel(s, c.id)) + (c.id === 'vitality' && s.allies.some(a => a.hp < a.maxHP * .4) ? 5 : 0);
    return score(b) - score(a);
  })[0];
  if (!card) return false;
  s.cards[card.id] = limit(legionCardLevel(s, card.id) + 1, 10000); s.pending--; refreshHealth(s);
  if (card.id === 'vitality') for (const a of s.allies) a.hp = Math.min(a.maxHP, a.hp + a.maxHP * .15);
  note(s, `新强化 · ${card.name} ${s.cards[card.id]}级`); return true;
}
function desiredKinds(s: LegionState): LegionUnitKind[] {
  const list: LegionUnitKind[] = ['guard', 'sword', 'archer', 'archer', 'guard', 'sword'];
  if (legionUnlocked(s, 'mage')) list.push('mage', s.strategy === 'elements' ? 'mage' : 'archer');
  if (legionUnlocked(s, 'priest')) list.push('priest', 'guard');
  if (legionUnlocked(s, 'giant')) list.push('giant', s.strategy === 'iron' ? 'giant' : s.strategy === 'elements' ? 'mage' : 'archer');
  if (s.bestWave >= 12) list.push('priest', s.strategy === 'iron' ? 'guard' : 'archer', 'sword', 'mage');
  return list;
}
function recruit(s: LegionState) {
  const desired = desiredKinds(s);
  for (const kind of desired) {
    const needed = desired.filter(k => k === kind).length;
    if (s.formation[kind] >= needed) continue;
    if (s.essence < unit(kind).cost) return;
    s.essence -= unit(kind).cost; addAlly(s, kind);
    if (legionCardLevel(s, 'recruit')) for (const a of s.allies) a.hp = Math.min(a.maxHP, a.hp + a.maxHP * .015 * legionCardLevel(s, 'recruit'));
    return;
  }
}
function nearest(a: LegionActor, targets: LegionActor[]) {
  let best: LegionActor | undefined, distance = Infinity;
  for (const b of targets) { if (b.hp <= 0) continue; const d = Math.hypot(a.x - b.x, (a.y - b.y) * 1.3); if (d < distance) { distance = d; best = b; } }
  return { target: best, distance };
}
function damage(s: LegionState, victim: LegionActor, raw: number) {
  if (victim.hp <= 0 || raw <= 0) return;
  let armor = victim.side === 'ally' ? legionStats(s, victim.kind as LegionUnitKind).armor : LEGION_ENEMIES[victim.kind as LegionEnemyKind].armor;
  if (victim.side === 'ally' && legionCardLevel(s, 'bulwark') && s.allies.some(a => a.kind === 'guard' && a.hp > 0 && Math.hypot(a.x - victim.x, a.y - victim.y) < 100)) armor += .087 * legionCardLevel(s, 'bulwark');
  const amount = Math.min(victim.hp, raw / (1 + armor)); victim.hp -= amount;
  if (victim.side === 'enemy') s.damage = limit(s.damage + amount);
}
function attack(s: LegionState, a: LegionActor, target: LegionActor) {
  const c = (id: LegionCardId) => legionCardLevel(s, id);
  if (a.side === 'enemy') {
    const base = LEGION_ENEMIES[a.kind as LegionEnemyKind], scale = (1 + (s.wave - 1) * .09) * Math.pow(1.018, Math.min(1200, s.wave - 1));
    damage(s, target, base.damage * scale); effect(s, base.range > 100 ? 'arrow' : 'slash', a, target);
    if (target.kind === 'guard' && c('thorns')) damage(s, a, base.damage * scale * .35 * c('thorns'));
    return;
  }
  const kind = a.kind as LegionUnitKind, st = legionStats(s, kind);
  let power = st.damage * (a.hp < a.maxHP * .5 ? 1 + c('resolve') * .2 : 1);
  if (kind === 'priest') {
    const injured = s.allies.filter(b => b.hp > 0 && b.hp < b.maxHP - .1).sort((x, y) => x.hp / x.maxHP - y.hp / y.maxHP);
    if (injured.length) { for (const b of injured.slice(0, 1 + Math.min(2, c('heal')))) { const amount = Math.min(b.maxHP - b.hp, power * (1 + c('heal') * .25)); b.hp += amount; s.healing = limit(s.healing + amount); effect(s, 'heal', a, b, `+${Math.round(amount)}`); } return; }
  }
  if (kind === 'archer') { power *= 1 + c('double') * .45 + (a.attack % 4 === 0 ? c('volley') * .6 : 0); if (bosses.includes(target.kind as LegionEnemyKind) || target.kind === 'armored') power *= 1 + c('focus') * .25; }
  if (kind === 'mage' && target.slow > 0) power *= 1 + c('shatter') * .3;
  damage(s, target, power); effect(s, kind === 'archer' ? 'arrow' : kind === 'mage' ? c('chain') ? 'lightning' : 'fire' : 'slash', a, target);
  effect(s, 'hit', target, target, `${Math.round(power)}`);
  if (kind === 'mage') { target.burn = Math.max(target.burn, power * c('burn') * .12); target.burnTime = c('burn') ? 3 : target.burnTime; if (c('frost')) target.slow = Math.max(target.slow, 2); }
  const nearby = s.enemies.filter(b => b.id !== target.id && b.hp > 0).sort((x, y) => Math.hypot(x.x - target.x, x.y - target.y) - Math.hypot(y.x - target.x, y.y - target.y));
  const radius = kind === 'mage' ? 65 + c('blast') * 15 : kind === 'giant' ? 65 + c('stomp') * 18 : kind === 'sword' && c('cleave') ? 55 : kind === 'archer' && c('explosive') ? 75 : 0;
  for (const b of nearby) if (Math.hypot(b.x - target.x, b.y - target.y) < radius) {
    damage(s, b, power * (kind === 'mage' ? .35 + c('blast') * .2 : kind === 'giant' ? .65 : kind === 'sword' ? c('cleave') * .4 : c('explosive') * .35));
    if (kind === 'giant' && c('stomp')) b.slow = 2;
  }
  const jumps = kind === 'mage' ? c('chain') : kind === 'archer' ? c('pierce') : 0;
  for (const b of nearby.filter(b => Math.hypot(b.x - target.x, b.y - target.y) < 210).slice(0, Math.min(nearby.length, jumps))) { damage(s, b, power * .55); effect(s, kind === 'mage' ? 'lightning' : 'arrow', target, b); }
}
function finishWave(s: LegionState) {
  s.cleared++; s.wins++; s.coins = limit(s.coins + 24 + s.wave * 3); s.essence = limit(s.essence + 18 + s.wave * 2);
  s.xp += 24 + s.wave * 2; s.bestRun = Math.max(s.bestRun, s.wave); s.wave++; s.bestWave = Math.max(s.bestWave, s.wave);
  s.phase = 'march'; s.phaseTime = 2.5; note(s, `守住战线 · 下一关 ${legionChapter(s)}，阵亡复活、存活者恢复30%生命`);
}
function defeat(s: LegionState) {
  s.defeats++; s.coins = limit(s.coins + 20 + s.wave * 2); s.essence = 60;
  // 随机构筑重置后回到可重新积累的战线，避免高层连续秒败。
  s.wave = Math.max(1, Math.floor((s.wave-1)*(s.defeats%3===0?.65:.75))); s.phase = 'recover'; s.phaseTime = 4;
  s.cards = {}; s.pending = 0; s.rank = 1; s.xp = 0; s.enemies = []; s.allies = []; s.effects = [];
  s.formation = {guard:0,sword:0,archer:0,mage:0,priest:0,giant:0};
  note(s, '远征结算 · 保留金币和训练，回退整备后自动续战');
}
export function restartLegion(s: LegionState) {
  const fresh = createLegion(s.seed); fresh.training = { ...s.training }; fresh.coins = s.coins; fresh.bestWave = s.bestWave; fresh.bestRun = s.bestRun;
  fresh.kills = s.kills; fresh.defeats = s.defeats; fresh.wins = s.wins; fresh.cleared = s.cleared; fresh.elapsed = s.elapsed; fresh.damage=s.damage;fresh.healing=s.healing;fresh.strategy = s.strategy; fresh.options = { ...s.options }; fresh.speed = s.speed;
  refreshHealth(fresh); return fresh;
}
export function stepLegion(s: LegionState, dt: number) {
  if (!Number.isFinite(dt) || dt <= 0 || dt > 1 || s.paused) return;
  s.elapsed += dt; s.effects = s.effects.filter(e => (e.life -= dt) > 0);
  if (s.options.strengthen && s.pending) strengthenLegion(s);
  const threshold = () => 45 + s.rank * 12;
  for (let count = 0; count < 5 && s.xp >= threshold(); count++) { s.xp -= threshold(); s.rank++; s.pending++; }
  s.trainClock += dt;
  if (s.options.train && s.trainClock >= 1) {
    s.trainClock = 0;
    const order = LEGION_UNITS.filter(u => legionUnlocked(s, u.id)).sort((a, b) => {
      const preference = (kind: LegionUnitKind) => s.training[kind] - ((s.strategy === 'arrows' && kind === 'archer') || (s.strategy === 'elements' && kind === 'mage') || (s.strategy === 'iron' && ['guard', 'giant'].includes(kind)) ? 2 : 0);
      return preference(a.id) - preference(b.id);
    });
    for (const u of order) if (trainLegion(s, u.id)) break;
  }
  if (s.phase !== 'battle') {
    if (s.phase === 'recover' && !s.options.retry) return;
    s.phaseTime -= dt; if (s.phaseTime > 0) return;
    if (s.phase === 'recover') for (const kind of desiredKinds(s)) addAlly(s, kind);
    beginWave(s); return;
  }
  s.waveTime += dt; s.recruitClock += dt; s.essence = limit(s.essence + dt * 2.4);
  if (s.recruitClock >= Math.max(.3, 1.8 / (1 + legionCardLevel(s, 'recruit') * .12))) {
    s.recruitClock = 0; if (s.options.summon) recruit(s);
  }
  s.spawnClock -= dt;
  if (s.spawnLeft > 0 && s.spawnClock <= 0) {
    const pool: LegionEnemyKind[] = s.wave < 3 ? ['goblin', 'swarm'] : s.wave < 5 ? ['goblin', 'runner', 'skeleton'] : ['goblin', 'runner', 'skeleton', 'armored', 'shaman', 'swarm'];
    if (addEnemy(s, pool[Math.floor(random(s) * pool.length)]!)) { s.spawnLeft--; s.spawnClock = Math.max(.35, 1.1 - s.wave * .02); }
  }
  for (const a of [...s.allies, ...s.enemies]) {
    if (a.hp <= 0) continue;
    if (a.burnTime > 0) { const burning = Math.min(a.burnTime, dt); a.burnTime = Math.max(0, a.burnTime - dt); damage(s, a, a.burn * burning); }
    if(a.hp<=0)continue;
    a.slow = Math.max(0, a.slow - dt); a.cooldown -= dt; a.skill -= dt;
    const foe = nearest(a, a.side === 'ally' ? s.enemies : s.allies); if (!foe.target) {a.cooldown=Math.max(0,a.cooldown);continue;}
    const base = a.side === 'ally' ? unit(a.kind as LegionUnitKind) : LEGION_ENEMIES[a.kind as LegionEnemyKind];
    const range = a.side === 'ally' ? legionStats(s, a.kind as LegionUnitKind).range : base.range;
    if (foe.distance > range) { const dx = foe.target.x - a.x, dy = foe.target.y - a.y, length = Math.max(1, Math.hypot(dx, dy)), amount = Math.min(foe.distance - range + 1, base.speed * dt * (a.slow ? .7 : 1)); a.x += dx / length * amount; a.y += dy / length * amount; a.cooldown=Math.max(0,a.cooldown); }
    else if (a.cooldown <= 0) {
      const rate=a.side==='ally'?legionStats(s,a.kind as LegionUnitKind).rate:base.rate;
      // 补足低频后台结算跨过的攻击间隔，与前台保持相同的攻速口径。
      const strikes=Math.min(12,Math.floor(-a.cooldown*rate)+1);
      for(let n=0;n<strikes&&foe.target.hp>0&&a.hp>0;n++){a.attack++;attack(s,a,foe.target);}
      a.cooldown+=strikes/rate;
    }
    if (a.side === 'enemy' && bosses.includes(a.kind as LegionEnemyKind) && a.skill <= 0) {
      a.skill = 7;
      if (a.kind === 'behemoth') { for (const b of s.allies.filter(b => Math.hypot(b.x - a.x, b.y - a.y) < 125)) { damage(s, b, LEGION_ENEMIES.behemoth.damage * (1 + s.wave * .1)); effect(s, 'fire', a, b); } note(s, '装甲巨兽 · 震地冲击'); }
      else { for (let i = 0; i < 3; i++) addEnemy(s, a.kind === 'necromancer' ? 'skeleton' : 'swarm', Math.min(895, a.x + 30 + i * 12), a.y + (i - 1) * 25); note(s, a.kind === 'necromancer' ? '亡灵祭司 · 唤回骷髅援军' : '蛮荒督军 · 战鼓召援'); }
    }
  }
  const rewarded=new Set<number>();
  for(let pass=0;pass<s.enemies.length;pass++) {
    const fallen=s.enemies.filter(a=>a.hp<=0&&!rewarded.has(a.id));if(!fallen.length)break;
    for(const a of fallen) {
      rewarded.add(a.id);s.kills++; const reward = 1 + legionCardLevel(s, 'bounty') * .15;
      s.coins = limit(s.coins + (4 + s.wave * .3) * reward); s.essence = limit(s.essence + 3 * reward); s.xp += 8;
      if (a.burnTime > 0 && legionCardLevel(s, 'inferno')) for (const b of s.enemies) if (b.hp > 0 && Math.hypot(b.x - a.x, b.y - a.y) < 80) damage(s, b, legionStats(s, 'mage').damage * legionCardLevel(s, 'inferno') * .3);
    }
  }
  s.enemies = s.enemies.filter(a => a.hp > 0); s.allies = s.allies.filter(a => a.hp > 0);
  // 同阵营轻量分离，避免多个近战角色重叠为一个点；无需逐像素寻路。
  for (const group of [s.allies,s.enemies]) for(let i=0;i<group.length;i++)for(let j=i+1;j<group.length;j++) {
    const a=group[i]!,b=group[j]!,dx=b.x-a.x,dy=b.y-a.y,d=Math.hypot(dx,dy);
    if(d>=24)continue; const push=(24-d)*.3,ny=d>.01?dy/d:(i%2?1:-1),nx=d>.01?dx/d:0;
    a.x=Math.max(40,Math.min(920,a.x-nx*push));b.x=Math.max(40,Math.min(920,b.x+nx*push));a.y=Math.max(214,Math.min(400,a.y-ny*push));b.y=Math.max(214,Math.min(400,b.y+ny*push));
  }
  if (!s.allies.length || s.waveTime > 120) defeat(s);
  else if (!s.spawnLeft && !s.enemies.length) finishWave(s);
}

/** 拒绝损坏、超量及非有限数值；军团采用独立存档，不修改其他游戏。 */
export function validLegion(value: unknown): value is LegionState {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  const s = value as LegionState, finite = (n: unknown, max = 1e12) => typeof n === 'number' && Number.isFinite(n) && n >= 0 && n <= max;
  if (s.version !== 1 || !LEGION_STRATEGIES.some(x => x.id === s.strategy) || !['battle', 'march', 'recover'].includes(s.phase) || typeof s.paused !== 'boolean' || ![1, 2].includes(s.speed)) return false;
  for (const k of ['seed', 'nextId', 'wave', 'bestWave', 'cleared', 'elapsed', 'phaseTime', 'waveTime', 'coins', 'essence', 'xp', 'rank', 'pending', 'spawnLeft', 'recruitClock', 'trainClock', 'kills', 'defeats', 'wins', 'bestRun', 'damage', 'healing'] as const) if (!finite(s[k])) return false;
  if (!Number.isSafeInteger(s.wave) || s.wave < 1 || s.bestWave < s.wave || !s.training || !s.formation || !s.cards || !s.options || s.spawnLeft > 28 || !Number.isFinite(s.spawnClock)) return false;
  if (!Object.values(s.options).every(v => typeof v === 'boolean') || !['summon', 'strengthen', 'replenish', 'retry', 'train'].every(k => typeof s.options[k as keyof Options] === 'boolean')) return false;
  if (!LEGION_UNITS.every(u => finite(s.training[u.id], 300) && Number.isInteger(s.training[u.id]))) return false;
  if (!LEGION_UNITS.every(u => finite(s.formation[u.id],24) && Number.isInteger(s.formation[u.id]))) return false;
  if (!Object.entries(s.cards).every(([k, v]) => LEGION_CARDS.some(c => c.id === k) && finite(v, 10000) && Number.isInteger(v))) return false;
  if (!Array.isArray(s.log) || s.log.length > 18 || !s.log.every(x => typeof x === 'string' && x.length <= 150)) return false;
  if (!Array.isArray(s.effects) || s.effects.length > LEGION_EFFECT_LIMIT || !s.effects.every(e => finite(e.id) && ['arrow', 'fire', 'lightning', 'heal', 'slash', 'hit'].includes(e.kind) && [e.x, e.y, e.tx, e.ty, e.life].every(Number.isFinite) && typeof e.text === 'string' && e.text.length < 40)) return false;
  if (!Array.isArray(s.allies) || !Array.isArray(s.enemies) || s.allies.length > 24 || s.allies.length + s.enemies.length > LEGION_ACTOR_LIMIT) return false;
  const ids = new Set<number>();
  for (const a of [...s.allies, ...s.enemies]) {
    if (!a || !['ally', 'enemy'].includes(a.side) || (a.side === 'ally' ? !LEGION_UNITS.some(u => u.id === a.kind) : !Object.hasOwn(LEGION_ENEMIES,a.kind))) return false;
    if (!finite(a.id) || a.id >= s.nextId || ids.has(a.id) || ![a.x, a.y, a.cooldown, a.skill].every(Number.isFinite)) return false;
    if (![a.hp, a.maxHP, a.attack, a.slow, a.burn, a.burnTime].every(n => finite(n)) || a.hp > a.maxHP || a.maxHP <= 0 || a.x < -100 || a.x > 1000 || a.y < 0 || a.y > 440) return false;
    ids.add(a.id);
  }
  return s.allies.every(a => a.side === 'ally') && s.enemies.every(a => a.side === 'enemy');
}
