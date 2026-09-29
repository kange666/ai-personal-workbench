export const SOKOBAN_LEVELS = [
  { name: "初次搬运", map: ["#######", "#     #", "# . $@#", "#     #", "#######"] },
  { name: "双向泊位", map: ["#######", "#  .  #", "#  $  #", "#     #", "# @$ .#", "#     #", "#######"] },
  { name: "绕过隔舱", map: ["########", "#   #  #", "# .    #", "# $$# .#", "#  @   #", "#      #", "########"] },
  { name: "有序入库", map: ["########", "#  ... #", "#      #", "# $$$  #", "#  #   #", "#  @   #", "########"] },
  { name: "分区调度", map: ["########", "# . # .#", "#   #  #", "# $$   #", "#   $ .#", "#  @   #", "########"] },
  { name: "最后一班", map: ["########", "# .  . #", "# #    #", "# $$$  #", "#    # #", "#   .@ #", "########"] },
] as const;

export type SokobanDirection = "up" | "right" | "down" | "left";
export const SOKOBAN_DIRECTIONS: Record<SokobanDirection, { dx: number; dy: number }> = {
  up: { dx: 0, dy: -1 }, right: { dx: 1, dy: 0 }, down: { dx: 0, dy: 1 }, left: { dx: -1, dy: 0 },
};
export interface SokobanBoard { width: number; height: number; walls: number[]; goals: number[]; player: number; boxes: number[] }
export interface SokobanSnapshot { player: number; boxes: number[]; moves: number; pushes: number }
export interface SokobanState extends SokobanSnapshot {
  version: 1;
  level: number;
  score: number;
  awards: number[];
  completed: boolean;
  history: SokobanSnapshot[];
}

export function getSokobanBoard(level: number): SokobanBoard {
  const map = SOKOBAN_LEVELS[level].map;
  const board: SokobanBoard = { width: map[0].length, height: map.length, walls: [], goals: [], player: -1, boxes: [] };
  map.forEach((row, y) => [...row].forEach((cell, x) => {
    const index = y * board.width + x;
    if (cell === "#") board.walls.push(index);
    if (cell === "." || cell === "+" || cell === "*") board.goals.push(index);
    if (cell === "$" || cell === "*") board.boxes.push(index);
    if (cell === "@" || cell === "+") board.player = index;
  }));
  return board;
}

export function createSokobanState(): SokobanState {
  const board = getSokobanBoard(0);
  return { version: 1, level: 0, player: board.player, boxes: [...board.boxes], moves: 0, pushes: 0, score: 0, awards: SOKOBAN_LEVELS.map(() => 0), completed: false, history: [] };
}

export function sokobanDestination(board: SokobanBoard, source: number, direction: SokobanDirection): number | null {
  const offset = SOKOBAN_DIRECTIONS[direction];
  if (!offset) return null;
  const x = source % board.width + offset.dx;
  const y = Math.floor(source / board.width) + offset.dy;
  if (x < 0 || x >= board.width || y < 0 || y >= board.height) return null;
  const target = y * board.width + x;
  return board.walls.includes(target) ? null : target;
}

export function isSokobanSolved(board: SokobanBoard, boxes: readonly number[]): boolean {
  return boxes.length === board.goals.length && boxes.every(box => board.goals.includes(box));
}

export function sokobanAward(moves: number, pushes: number): number {
  return Math.max(100, 1000 - moves * 5 - pushes * 10);
}

export function moveSokobanSnapshot(board: SokobanBoard, state: SokobanSnapshot, direction: SokobanDirection): SokobanSnapshot | null {
  const destination = sokobanDestination(board, state.player, direction);
  if (destination === null) return null;
  const boxIndex = state.boxes.indexOf(destination);
  const boxes = [...state.boxes];
  if (boxIndex !== -1) {
    const beyond = sokobanDestination(board, destination, direction);
    if (beyond === null || boxes.includes(beyond)) return null;
    boxes[boxIndex] = beyond;
  }
  return { player: destination, boxes, moves: state.moves + 1, pushes: state.pushes + (boxIndex === -1 ? 0 : 1) };
}

export function moveSokoban(state: SokobanState, direction: SokobanDirection): SokobanState {
  if (state.completed) return state;
  const board = getSokobanBoard(state.level);
  const next = moveSokobanSnapshot(board, state, direction);
  if (!next) return state;
  const completed = isSokobanSolved(board, next.boxes);
  const awards = [...state.awards];
  if (completed) awards[state.level] = sokobanAward(next.moves, next.pushes);
  const snapshot: SokobanSnapshot = { player: state.player, boxes: [...state.boxes], moves: state.moves, pushes: state.pushes };
  // 保存最近 200 步，避免长时间游玩把本机存档无限增大。
  return { ...state, ...next, awards, score: awards.reduce((sum, award) => sum + award, 0), completed, history: [...state.history.slice(-199), snapshot] };
}

export function undoSokoban(state: SokobanState): SokobanState {
  const previous = state.history.at(-1);
  if (!previous) return state;
  const awards = [...state.awards];
  awards[state.level] = 0;
  return { ...state, ...previous, boxes: [...previous.boxes], awards, score: awards.reduce((sum, award) => sum + award, 0), completed: false, history: state.history.slice(0, -1) };
}

export function restartSokobanLevel(state: SokobanState): SokobanState {
  const board = getSokobanBoard(state.level);
  const awards = [...state.awards];
  awards[state.level] = 0;
  return { ...state, player: board.player, boxes: [...board.boxes], moves: 0, pushes: 0, awards, score: awards.reduce((sum, award) => sum + award, 0), completed: false, history: [] };
}

export function nextSokobanLevel(state: SokobanState): SokobanState {
  if (!state.completed || state.level >= SOKOBAN_LEVELS.length - 1) return state;
  return restartSokobanLevel({ ...state, level: state.level + 1 });
}

function nonnegativeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

function validSnapshot(value: unknown, board: SokobanBoard): value is SokobanSnapshot {
  if (!value || typeof value !== "object") return false;
  const snapshot = value as Partial<SokobanSnapshot>;
  const validPosition = (position: unknown): position is number => nonnegativeInteger(position) && position < board.width * board.height && !board.walls.includes(position);
  return validPosition(snapshot.player) && Array.isArray(snapshot.boxes) && snapshot.boxes.length === board.boxes.length
    && snapshot.boxes.every(validPosition) && new Set(snapshot.boxes).size === snapshot.boxes.length && !snapshot.boxes.includes(snapshot.player)
    && nonnegativeInteger(snapshot.moves) && nonnegativeInteger(snapshot.pushes) && snapshot.pushes <= snapshot.moves;
}

function equalBoxes(left: readonly number[], right: readonly number[]): boolean {
  return left.length === right.length && left.every(box => right.includes(box));
}

export function validateSokobanState(value: unknown): value is SokobanState {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<SokobanState>;
  if (candidate.version !== 1 || !nonnegativeInteger(candidate.level) || candidate.level >= SOKOBAN_LEVELS.length) return false;
  const level = candidate.level;
  const board = getSokobanBoard(level);
  if (!validSnapshot(value, board)) return false;
  const state = value as SokobanSnapshot & Partial<SokobanState>;
  if (!nonnegativeInteger(state.score) || typeof state.completed !== "boolean"
    || !Array.isArray(state.awards) || state.awards.length !== SOKOBAN_LEVELS.length
    || !state.awards.every((award, index) => nonnegativeInteger(award) && award <= 1000 && award % 5 === 0 && (index < level ? award >= 100 : index > level ? award === 0 : true))
    || state.score !== state.awards.reduce((sum, award) => sum + award, 0)
    || state.completed !== isSokobanSolved(board, state.boxes)
    || state.awards[level] !== (state.completed ? sokobanAward(state.moves, state.pushes) : 0)
    || !Array.isArray(state.history) || state.history.length > 200 || !state.history.every(snapshot => validSnapshot(snapshot, board))) return false;
  if (state.moves === 0 && (state.player !== board.player || !equalBoxes(state.boxes, board.boxes) || state.history.length !== 0)) return false;
  if (state.history.length > state.moves) return false;
  const snapshots = [...state.history, state];
  return snapshots.every((snapshot, index) => {
    if (index === 0) return true;
    const previous = snapshots[index - 1];
    if (isSokobanSolved(board, previous.boxes)) return false;
    return (Object.keys(SOKOBAN_DIRECTIONS) as SokobanDirection[]).some(direction => {
      const moved = moveSokobanSnapshot(board, previous, direction);
      return moved !== null && moved.player === snapshot.player && moved.moves === snapshot.moves && moved.pushes === snapshot.pushes && equalBoxes(moved.boxes, snapshot.boxes);
    });
  });
}
