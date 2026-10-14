import { createTower, dropTower, isTowerState, type TowerState } from './tower';
import { finite, integer, object } from './progression';
export const TOWER_LEVELS = Array.from({ length: 40 }, (_, i) => ({
  name: `${['稳稳落下','保留宽度','完美节奏','缓急之间','逆风起楼','连击工坊','云端施工','星空之城'][Math.floor(i / 5)]} ${i % 5 + 1}`,
  chapter: `第 ${Math.floor(i / 5) + 1} 章`, height: 6 + Math.floor(i / 2),
  width: i % 5 === 1 ? 85 : i % 5 === 4 ? 60 : 1,
  perfect: i % 5 === 2 ? 2 + Math.floor(i / 15) : 0,
  pattern: Math.floor(i / 5) % 4,
}));
export interface TowerAdventure {
  version: 1; run: TowerState; level: number; mode: 'campaign' | 'endless'; won: boolean;
  time: number; blockTime: number; reversed: boolean; perfects: number; maxStreak: number;
  slow: number; repair: number; undo: number; slowUntil: number; used: number;
  previous: { run: Omit<TowerState,'blocks'>; top: {x:number;width:number}; length:number; perfects: number; maxStreak: number } | null;
}
export function createTowerAdventure(level = 0, mode: 'campaign' | 'endless' = 'campaign'): TowerAdventure {
  return { version: 1, run: createTower(), level, mode, won: false, time: 0, blockTime: 0, reversed: false, perfects: 0, maxStreak: 0, slow: 2, repair: 1, undo: 1, slowUntil: 0, used: 0, previous: null };
}
export function stepTowerAdventure(state: TowerAdventure, seconds: number) {
  if (state.won || state.run.phase !== 'moving' || !finite(seconds, 0, 1)) return;
  const dt = Math.min(seconds, .05), level = TOWER_LEVELS[state.level];
  state.time += dt; state.blockTime += dt;
  const pattern = state.mode === 'endless' ? 0 : level.pattern;
  let multiplier = pattern === 1 ? .75 + .35 * Math.sin(state.blockTime * 2) : pattern === 2 ? (state.blockTime % 3 < .45 ? .3 : 1) : 1;
  // 高级关每块只反向一次，画面给出提示；开局和最初几关保持匀速。
  if (pattern === 3 && !state.reversed && state.blockTime > 1.5) { state.run.direction *= -1; state.reversed = true; }
  if (state.time < state.slowUntil) multiplier *= .45;
  const speed = Math.min(260, 90 + (state.run.blocks.length - 1) * 4 + (state.mode === 'campaign' ? state.level * 1.5 : 0));
  const end = 480 - state.run.blocks.at(-1)!.width;
  state.run.movingX += state.run.direction * speed * multiplier * dt;
  if (state.run.movingX > end) { state.run.movingX = Math.max(0, 2 * end - state.run.movingX); state.run.direction = -1; }
  if (state.run.movingX < 0) { state.run.movingX = Math.min(end, -state.run.movingX); state.run.direction = 1; }
}
export function dropTowerAdventure(state: TowerAdventure) {
  if (state.won || state.run.phase !== 'moving') return null;
  // 回退只保存上一层的差异，避免万层无尽局把整座高塔复制到存档里。
  const {blocks,...run}=state.run;
  state.previous = state.undo > 0 ? { run: {...run}, top: {...blocks.at(-1)!}, length:blocks.length, perfects: state.perfects, maxStreak: state.maxStreak } : null;
  const result = dropTower(state.run);
  state.blockTime = 0; state.reversed = false;
  if (result?.kind === 'perfect') state.perfects++;
  state.maxStreak = Math.max(state.maxStreak, state.run.perfectStreak);
  const goal = TOWER_LEVELS[state.level];
  if (state.mode === 'campaign' && state.run.blocks.length - 1 >= goal.height && state.run.blocks.at(-1)!.width >= goal.width && state.maxStreak >= goal.perfect) state.won = true;
  return result;
}
export function towerTool(state: TowerAdventure, kind: 'slow' | 'repair' | 'undo'): boolean {
  if (state.won || state[kind] <= 0 || !['moving','over'].includes(state.run.phase)) return false;
  if (kind === 'undo') {
    if (!state.previous) return false;
    const previous=state.previous,blocks=state.run.blocks.slice(0,previous.length);blocks[previous.length-1]={...previous.top};
    state.run = {...previous.run,blocks}; state.perfects = previous.perfects; state.maxStreak = previous.maxStreak; state.previous = null;
  } else if (state.run.phase !== 'moving') return false;
  else if (kind === 'slow') state.slowUntil = state.time + 6;
  else {
    const top = state.run.blocks.at(-1)!, below = state.run.blocks.at(-2);
    if (!below || top.width >= below.width) return false;
    const width = Math.min(below.width, top.width + 40);
    top.x = Math.max(below.x, Math.min(top.x - (width - top.width) / 2, below.x + below.width - width)); top.width = width;
    state.run.movingX = Math.min(state.run.movingX, 480 - width);
  }
  state[kind]--; state.used++; state.blockTime = 0; state.reversed = false; return true;
}
export function validTowerAdventure(value: unknown): value is TowerAdventure {
  if (!object(value)) return false;
  const s = value as unknown as TowerAdventure;
  return s.version === 1 && isTowerState(s.run) && integer(s.level, 0, 39) && ['campaign','endless'].includes(s.mode)
    && typeof s.won === 'boolean' && finite(s.time, 0, 1e12) && finite(s.blockTime, 0, 1e12) && typeof s.reversed === 'boolean'
    && integer(s.perfects, 0, s.run.blocks.length) && integer(s.maxStreak, 0, s.run.blocks.length) && integer(s.used, 0, 4)
    && integer(s.slow, 0, 2) && integer(s.repair, 0, 1) && integer(s.undo, 0, 1) && finite(s.slowUntil, 0, 1e12)
    && (s.previous === null || (object(s.previous) && object(s.previous.run) && integer(s.previous.length,1,s.run.blocks.length) && s.previous.length>=s.run.blocks.length-1 && object(s.previous.top) && isTowerState({...s.previous.run,blocks:[...s.run.blocks.slice(0,s.previous.length-1),s.previous.top]}) && integer(s.previous.perfects, 0, 10000) && integer(s.previous.maxStreak, 0, 10000)))
    && (!s.won || (s.mode === 'campaign' && s.run.blocks.length - 1 >= TOWER_LEVELS[s.level].height && s.run.blocks.at(-1)!.width >= TOWER_LEVELS[s.level].width && s.maxStreak >= TOWER_LEVELS[s.level].perfect));
}
