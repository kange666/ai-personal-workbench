export const WIRING_SIZE = 5;
export const WIRING_SOURCE = 12;
export const WIRE_DIRECTIONS = [
  { bit: 1, opposite: 4, dx: 0, dy: -1 },
  { bit: 2, opposite: 8, dx: 1, dy: 0 },
  { bit: 4, opposite: 1, dx: 0, dy: 1 },
  { bit: 8, opposite: 2, dx: -1, dy: 0 },
] as const;

export interface WiringState {
  version: 1;
  round: number;
  tiles: number[];
  solution: number[];
  turns: number;
  score: number;
  won: boolean;
  lastAward: number;
}

export function rotateWire(mask: number): number {
  return ((mask << 1) & 15) | (mask >> 3);
}

function neighbor(index: number, dx: number, dy: number): number | null {
  const x = index % WIRING_SIZE + dx;
  const y = Math.floor(index / WIRING_SIZE) + dy;
  return x < 0 || y < 0 || x >= WIRING_SIZE || y >= WIRING_SIZE ? null : y * WIRING_SIZE + x;
}

export function wiringPower(tiles: readonly number[]): Set<number> {
  const powered = new Set([WIRING_SOURCE]);
  const queue = [WIRING_SOURCE];
  for (let cursor = 0; cursor < queue.length; cursor++) {
    const current = queue[cursor];
    for (const direction of WIRE_DIRECTIONS) {
      if (!(tiles[current] & direction.bit)) continue;
      const target = neighbor(current, direction.dx, direction.dy);
      if (target === null || !(tiles[target] & direction.opposite) || powered.has(target)) continue;
      powered.add(target);
      queue.push(target);
    }
  }
  return powered;
}

export function wiringLeaks(tiles: readonly number[]): number {
  return tiles.reduce((count, mask, index) => count + WIRE_DIRECTIONS.filter(direction => {
    if (!(mask & direction.bit)) return false;
    const target = neighbor(index, direction.dx, direction.dy);
    return target === null || !(tiles[target] & direction.opposite);
  }).length, 0);
}

export function isWiringSolved(tiles: readonly number[]): boolean {
  return tiles.length === WIRING_SIZE ** 2 && wiringLeaks(tiles) === 0 && wiringPower(tiles).size === tiles.length;
}

export function createWiringState(random: () => number = Math.random, round = 1, score = 0): WiringState {
  // 先生成覆盖全部格子的连通树，再旋转线路，始终保留一个可还原的解。
  const solution = Array<number>(WIRING_SIZE ** 2).fill(0);
  const seen = new Set([WIRING_SOURCE]);
  const stack = [WIRING_SOURCE];
  const pick = (length: number) => Math.min(length - 1, Math.max(0, Math.floor(random() * length) || 0));
  while (stack.length) {
    const current = stack[stack.length - 1];
    const available = WIRE_DIRECTIONS.map(direction => ({ direction, target: neighbor(current, direction.dx, direction.dy) }))
      .filter((entry): entry is { direction: typeof WIRE_DIRECTIONS[number]; target: number } => entry.target !== null && !seen.has(entry.target));
    if (!available.length) { stack.pop(); continue; }
    const { direction, target } = available[pick(available.length)];
    solution[current] |= direction.bit;
    solution[target] |= direction.opposite;
    seen.add(target);
    stack.push(target);
  }
  const tiles = solution.map(mask => {
    for (let turns = pick(4); turns > 0; turns--) mask = rotateWire(mask);
    return mask;
  });
  if (isWiringSolved(tiles)) {
    const leaf = tiles.findIndex(mask => (mask & (mask - 1)) === 0);
    tiles[leaf] = rotateWire(tiles[leaf]);
  }
  return { version: 1, round, tiles, solution, turns: 0, score, won: false, lastAward: 0 };
}

export function wiringAward(turns: number): number {
  return Math.max(100, 1000 - turns * 10);
}

export function turnWire(state: WiringState, index: number): WiringState {
  if (state.won || !Number.isInteger(index) || index < 0 || index >= state.tiles.length) return state;
  const tiles = [...state.tiles];
  tiles[index] = rotateWire(tiles[index]);
  const turns = state.turns + 1;
  const won = isWiringSolved(tiles);
  const lastAward = won ? wiringAward(turns) : 0;
  return { ...state, tiles, turns, won, lastAward, score: state.score + lastAward };
}

export function nextWiringRound(state: WiringState, random: () => number = Math.random): WiringState {
  return state.won ? createWiringState(random, state.round + 1, state.score) : state;
}

function safeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

function wireArray(value: unknown): value is number[] {
  return Array.isArray(value) && value.length === WIRING_SIZE ** 2 && value.every(mask => Number.isInteger(mask) && mask > 0 && mask < 16);
}

export function validateWiringState(value: unknown): value is WiringState {
  if (!value || typeof value !== "object") return false;
  const state = value as Partial<WiringState>;
  if (state.version !== 1 || !safeInteger(state.round) || state.round < 1 || !safeInteger(state.turns)
    || !safeInteger(state.score) || !safeInteger(state.lastAward) || typeof state.won !== "boolean"
    || !wireArray(state.tiles) || !wireArray(state.solution) || !isWiringSolved(state.solution)) return false;
  if (!state.tiles.every((mask, index) => {
    let original = state.solution![index];
    for (let turn = 0; turn < 4; turn++, original = rotateWire(original)) if (mask === original) return true;
    return false;
  })) return false;
  const completedRounds = state.round - (state.won ? 0 : 1);
  return state.won === isWiringSolved(state.tiles)
    && state.lastAward === (state.won ? wiringAward(state.turns) : 0)
    && state.score >= state.lastAward && state.score % 10 === 0
    && state.score >= completedRounds * 100 && state.score <= completedRounds * 1000;
}
