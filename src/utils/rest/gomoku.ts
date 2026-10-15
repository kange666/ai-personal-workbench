import { searchGomokuExpert } from './gomokuExpert';
export const GOMOKU_SIZE = 15;
export type Stone = 0 | 1 | 2;
export type GomokuStatus = "playing" | "won" | "lost" | "draw";
export interface GomokuMove { index: number; player: 1 | 2 }
export interface GomokuState { board: Stone[]; moves: GomokuMove[]; status: GomokuStatus; score: number; human?: 1 | 2 }
const directions = [[1, 0], [0, 1], [1, 1], [1, -1]] as const;

export function createGomoku(human: 1 | 2 = 1): GomokuState {
  return { board: Array<Stone>(GOMOKU_SIZE * GOMOKU_SIZE).fill(0), moves: [], status: "playing", score: 0, human };
}

function inside(x: number, y: number): boolean { return x >= 0 && y >= 0 && x < GOMOKU_SIZE && y < GOMOKU_SIZE; }

export function winningLine(board: Stone[], index: number): number[] {
  const player = board[index];
  if (!player) return [];
  const x = index % GOMOKU_SIZE, y = Math.floor(index / GOMOKU_SIZE);
  for (const [dx, dy] of directions) {
    const line = [index];
    for (const sign of [-1, 1]) {
      let nx = x + dx * sign, ny = y + dy * sign;
      while (inside(nx, ny) && board[ny * GOMOKU_SIZE + nx] === player) {
        line.push(ny * GOMOKU_SIZE + nx); nx += dx * sign; ny += dy * sign;
      }
    }
    if (line.length >= 5) return line;
  }
  return [];
}

export function placeGomoku(state: GomokuState, index: number, player: 1 | 2): boolean {
  if (state.status !== "playing" || !Number.isInteger(index) || index < 0 || index >= state.board.length
    || state.board[index] !== 0 || player !== (state.moves.length % 2 === 0 ? 1 : 2)) return false;
  state.board[index] = player;
  state.moves.push({ index, player });
  if (winningLine(state.board, index).length) {
    state.status = player === (state.human || 1) ? "won" : "lost";
    state.score = player === (state.human || 1) ? Math.max(50, 500 - Math.ceil(state.moves.length / 2) * 10) : 0;
  } else if (state.moves.length === state.board.length) { state.status = "draw"; state.score = 50; }
  return true;
}

export function undoGomoku(state: GomokuState): boolean {
  // 已结算的棋局不可悔棋，防止同一局反复结算得分。
  if (state.status !== "playing" || !state.moves.length) return false;
  const floor = state.human === 2 ? 1 : 0;
  const count = Math.min(state.moves.length - floor, (state.moves.length % 2 + 1) === (state.human || 1) ? 2 : 1);
  if (count <= 0) return false;
  for (let i = 0; i < count; i += 1) {
    const move = state.moves.pop();
    if (move) state.board[move.index] = 0;
  }
  return true;
}

export function isGomokuState(value: unknown): value is GomokuState {
  if (!value || typeof value !== "object") return false;
  const s = value as GomokuState;
  if (!Array.isArray(s.board) || s.board.length !== 225 || s.board.some(cell => ![0, 1, 2].includes(cell))
    || !Array.isArray(s.moves) || s.moves.length > 225 || !["playing", "won", "lost", "draw"].includes(s.status)) return false;
  if (s.human !== undefined && ![1,2].includes(s.human)) return false;
  const reconstructed = createGomoku(s.human || 1);
  for (const move of s.moves) {
    if (!move || typeof move !== "object" || ![1, 2].includes(move.player) || !placeGomoku(reconstructed, move.index, move.player)) return false;
  }
  return s.status === reconstructed.status && s.score === reconstructed.score && s.board.every((stone, index) => stone === reconstructed.board[index]);
}

function lineValue(board: Stone[], index: number, player: 1 | 2): number {
  const x = index % GOMOKU_SIZE, y = Math.floor(index / GOMOKU_SIZE);
  let total = 0;
  for (const [dx, dy] of directions) {
    let length = 1, open = 0;
    for (const sign of [-1, 1]) {
      let nx = x + dx * sign, ny = y + dy * sign;
      while (inside(nx, ny) && board[ny * GOMOKU_SIZE + nx] === player) { length += 1; nx += dx * sign; ny += dy * sign; }
      if (inside(nx, ny) && board[ny * GOMOKU_SIZE + nx] === 0) open += 1;
    }
    if (length >= 5) total += 1_000_000;
    else if (open === 2) total += [0, 3, 50, 1800, 80_000][length]!;
    else if (open === 1) total += [0, 1, 8, 100, 9000][length]!;
  }
  return total;
}

export type GomokuDifficulty = "relaxed" | "normal" | "hard" | "expert" | "deepseek";
export const GOMOKU_DIFFICULTIES = [{id:"relaxed",name:"入门 · 轻松下"},{id:"normal",name:"普通"},{id:"hard",name:"困难"},{id:"expert",name:"高手"},{id:"deepseek",name:"DeepSeek · 最高"}] as const;
export function chooseGomokuMove(board: Stone[], difficulty: Exclude<GomokuDifficulty, 'deepseek'> = "hard", random: () => number = Math.random, player: 1 | 2 = 2): number | null {
  if (board.length !== 225 || board.every(cell => cell !== 0)) return null;
  if (board.every(cell => cell === 0)) return 112;
  if (difficulty === 'expert') return searchGomokuExpert(board, player, { timeMs: 300 }).index;
  const candidates: number[] = [];
  board.forEach((stone, index) => {
    if (stone) return;
    const x = index % GOMOKU_SIZE, y = Math.floor(index / GOMOKU_SIZE);
    for (let dy = -2; dy <= 2; dy += 1) for (let dx = -2; dx <= 2; dx += 1) {
      if (inside(x + dx, y + dy) && board[(y + dy) * GOMOKU_SIZE + x + dx]) { candidates.push(index); return; }
    }
  });
  let best = candidates[0] ?? null, bestValue = -Infinity;
  // 先取胜，再挡对方的一步成五；其余只评估邻近空位，保持落子响应轻快。
  const opponent: 1 | 2 = player === 1 ? 2 : 1;
  for (const side of [player, opponent] as const) {
    // 入门电脑会看漏部分威胁；主动进攻仍保留，避免变成纯随机落子。
    if (side === opponent && difficulty === "relaxed" && random() < .38) continue;
    for (const index of candidates) {
      board[index] = side;
      const wins = winningLine(board, index).length > 0;
      board[index] = 0;
      if (wins) return index;
    }
  }
  const ranked: {index:number; value:number}[] = [];
  for (const index of candidates) {
    const attack = lineValue(board, index, player), defense = lineValue(board, index, opponent);
    const distance = Math.abs(index % 15 - 7) + Math.abs(Math.floor(index / 15) - 7);
    const value = attack + defense * (difficulty === "relaxed" ? .22 : difficulty === "normal" ? .65 : 1.08) - distance * 0.1;
    ranked.push({index,value});
    if (value > bestValue) { bestValue = value; best = index; }
  }
  ranked.sort((a,b)=>b.value-a.value);
  if (difficulty === "relaxed") return ranked[Math.min(ranked.length-1,Math.floor(random()*Math.min(9,ranked.length)))]?.index ?? best;
  if (difficulty === "normal" && random() < .25) return ranked[Math.min(2,ranked.length-1)]?.index ?? best;
  return best;
}
