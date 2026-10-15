import { createGomoku, isGomokuState, placeGomoku, winningLine, type GomokuDifficulty, type GomokuState } from './gomoku';
import { GOMOKU_POSITIONS } from './gomokuPuzzles';
import { createProgress, object, validProgress, type GameProgress } from './progression';

export const GOMOKU_PUZZLES = GOMOKU_POSITIONS.map(position => ({
  name: position.name, chapter: position.chapter,
  turns: Math.ceil(position.solution.length / 2),
  human: (position.moves.length % 2 + 1) as 1 | 2,
}));
export function createGomokuPuzzle(index: number) {
  const puzzle = GOMOKU_POSITIONS[index] || GOMOKU_POSITIONS[0]!;
  const run = createGomoku((puzzle.moves.length % 2 + 1) as 1 | 2);
  for (const cell of puzzle.moves) placeGomoku(run, cell, (run.moves.length % 2 + 1) as 1 | 2);
  return { run, answer: puzzle.solution[0], solution: [...puzzle.solution] };
}

/** 提示/电脑参考题解；偏离参考杀法后仍正常对弈，只有实际连五才判通关。 */
export function gomokuPuzzleContinuation(index: number, run: GomokuState): number | null {
  const puzzle = GOMOKU_POSITIONS[index];
  if (!puzzle || run.moves.length < puzzle.moves.length) return null;
  const reference: readonly number[] = [...puzzle.moves, ...puzzle.solution];
  if (!run.moves.every((move, i) => reference[i] === move.index)) return null;
  return reference[run.moves.length] ?? null;
}
export function gomokuPuzzleTurns(index: number, run: GomokuState) {
  const initial = GOMOKU_POSITIONS[index]?.moves.length || 0;
  return run.moves.slice(initial).filter(move => move.player === (run.human || 1)).length;
}
// 保留给题库检查使用；界面不再以“一步等于预设答案”代替完整解题过程。
export function correctGomokuPuzzle(index: number, before: GomokuState, move: number) {
  if (before.board[move] !== 0) return false;
  if (gomokuPuzzleContinuation(index, before) === move) return true;
  const board = [...before.board]; board[move] = before.human || 1;
  return winningLine(board, move).length >= 5;
}

export interface GomokuSession { version:3; run:GomokuState; difficulty:GomokuDifficulty; mode:'endless'|'puzzle'; level:number; done:boolean; hintUsed?:boolean; progress:GameProgress }
export function createGomokuSession(): GomokuSession {
  return { version:3, run:createGomoku(), difficulty:'relaxed', mode:'endless', level:0, done:false, hintUsed:false, progress:createProgress(30) };
}
export function validGomokuSession(v:unknown):v is GomokuSession {
  return object(v) && v.version===3 && isGomokuState(v.run) && ['relaxed','normal','hard','expert','deepseek'].includes(v.difficulty as string)
    && ['endless','puzzle'].includes(v.mode as string) && Number.isInteger(v.level) && Number(v.level)>=0 && Number(v.level)<30
    && typeof v.done==='boolean' && (v.hintUsed===undefined||typeof v.hintUsed==='boolean') && validProgress(v.progress,30);
}
export function migrateGomoku(v:unknown): GomokuSession|null {
  if (isGomokuState(v)) return { ...createGomokuSession(), run:v };
  if (!object(v) || v.version!==2 || !validGomokuSession({...v,version:3})) return null;
  const next = {...v,version:3} as GomokuSession;
  // 新题库的题目/目标已变更，旧题星级不能冒充新题已解；自由对弈和外层最高分保留。
  next.progress=createProgress(30); next.level=0; next.done=false; next.hintUsed=false;
  if (next.mode==='puzzle') next.run=createGomokuPuzzle(0).run;
  return next;
}
