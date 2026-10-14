export const clamp = (n: number, low: number, high: number) => Math.max(low, Math.min(high, n));
export const finite = (n: unknown, low = -1e9, high = 1e9): n is number => typeof n === "number" && Number.isFinite(n) && n >= low && n <= high;
export const integer = (n: unknown, low = 0, high = 1e9): n is number => finite(n, low, high) && Number.isSafeInteger(n);
export const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === "object" && !Array.isArray(value);
/** 种子随存档保存，读档不会重新抽取风向、敌人或强化选项。 */
export function random(state: { seed: number }) { state.seed = (Math.imul(state.seed, 1664525) + 1013904223) >>> 0; return state.seed / 4294967296; }
export function seeded(seed: number) { const state = { seed: seed >>> 0 }; return () => random(state); }
export interface GameProgress { version: 1; stars: number[]; bestMoves: number[]; unlocked: number }
export function createProgress(count: number): GameProgress { return { version: 1, stars: Array(count).fill(0), bestMoves: Array(count).fill(0), unlocked: 0 }; }
export function validProgress(value: unknown, count: number): value is GameProgress {
  if (!object(value)) return false;
  return value.version === 1 && integer(value.unlocked, 0, count - 1) && Array.isArray(value.stars) && value.stars.length === count && value.stars.every(n => integer(n, 0, 3))
    && Array.isArray(value.bestMoves) && value.bestMoves.length === count && value.bestMoves.every(n => integer(n));
}
export function finishLevel(progress: GameProgress, level: number, stars: number, moves = 0) {
  if (!integer(level, 0, progress.stars.length - 1)) return;
  progress.stars[level] = Math.max(progress.stars[level]!, clamp(Math.floor(stars), 1, 3));
  if (moves > 0) progress.bestMoves[level] = progress.bestMoves[level] ? Math.min(progress.bestMoves[level]!, moves) : moves;
  progress.unlocked = Math.max(progress.unlocked, Math.min(progress.stars.length - 1, level + 1));
}
