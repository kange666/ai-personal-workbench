export const FISH_SPECIES = [
  { name: "银鳞鱼", color: "#b8dce7", min: 0.3, max: 1.2, value: 45 },
  { name: "青尾鲈", color: "#79c6b7", min: 0.8, max: 2.8, value: 60 },
  { name: "蜜金鲤", color: "#edc36c", min: 1, max: 3.6, value: 75 },
  { name: "月光鳟", color: "#bbb6ee", min: 0.6, max: 2.4, value: 100 },
  { name: "星斑鱼", color: "#e99fae", min: 1.5, max: 4.5, value: 125 },
] as const;

export type FishingPhase = "idle" | "waiting" | "bite" | "reeling" | "caught" | "escaped";
export type Fish = { species: number; weight: number };
export interface FishingState {
  phase: FishingPhase;
  elapsed: number;
  waitSeconds: number;
  fish: Fish | null;
  reelElapsed: number;
  reelHits: number;
  lastPull: number;
  score: number;
  caught: number;
  collection: number[];
  lastPoints: number;
  escapeReason: "late" | "miss" | "";
}

export function createFishing(): FishingState {
  return { phase: "idle", elapsed: 0, waitSeconds: 2, fish: null, reelElapsed: 0, reelHits: 0, lastPull: -1, score: 0, caught: 0, collection: FISH_SPECIES.map(() => 0), lastPoints: 0, escapeReason: "" };
}

const finite = (value: unknown, min: number, max: number): value is number => typeof value === "number" && Number.isFinite(value) && value >= min && value <= max;
const integer = (value: unknown, min: number, max: number): value is number => finite(value, min, max) && Number.isInteger(value);

export function isFishingState(value: unknown): value is FishingState {
  if (!value || typeof value !== "object") return false;
  const s = value as FishingState;
  if (!["idle", "waiting", "bite", "reeling", "caught", "escaped"].includes(s.phase)
    || !finite(s.elapsed, 0, 5) || !finite(s.waitSeconds, 1.8, 4.2)
    || !finite(s.reelElapsed, 0, 10) || !integer(s.reelHits, 0, 3)
    || !finite(s.lastPull, -1, s.reelElapsed) || !integer(s.score, 0, 1_000_000_000)
    || !integer(s.caught, 0, 10_000_000) || !integer(s.lastPoints, 0, 1000)
    || !["late", "miss", ""].includes(s.escapeReason)
    || !Array.isArray(s.collection) || s.collection.length !== FISH_SPECIES.length
    || s.collection.some(count => !integer(count, 0, s.caught))
    || s.collection.reduce((sum, count) => sum + count, 0) !== s.caught) return false;
  const minScore = s.collection.reduce((sum, count, index) => sum + count * Math.round(FISH_SPECIES[index]!.min * FISH_SPECIES[index]!.value), 0);
  const maxScore = s.collection.reduce((sum, count, index) => sum + count * Math.round(FISH_SPECIES[index]!.max * FISH_SPECIES[index]!.value), 0);
  if (s.score < minScore || s.score > maxScore || s.lastPoints > s.score) return false;
  if (s.phase === "idle") return s.fish === null && s.reelHits === 0 && s.caught === 0 && s.score === 0 && s.elapsed === 0 && s.reelElapsed === 0;
  if (!s.fish || !integer(s.fish.species, 0, FISH_SPECIES.length - 1)) return false;
  const species = FISH_SPECIES[s.fish.species]!;
  if (!finite(s.fish.weight, species.min, species.max)) return false;
  if (s.phase === "waiting" && (s.elapsed >= s.waitSeconds || s.reelHits !== 0)) return false;
  if (s.phase === "bite" && (s.elapsed >= 2.4 || s.reelHits !== 0)) return false;
  if (s.phase === "reeling" && (s.reelHits >= 3 || s.reelElapsed >= 9)) return false;
  if (s.phase === "caught" && (s.reelHits !== 3 || s.lastPoints !== fishPoints(s.fish) || s.collection[s.fish.species]! < 1)) return false;
  if (s.phase === "escaped" ? !s.escapeReason : s.escapeReason !== "") return false;
  if (s.phase !== "caught" && s.lastPoints !== 0) return false;
  return true;
}

export function fishPoints(fish: Fish): number {
  return Math.round(FISH_SPECIES[fish.species]!.value * fish.weight);
}

export function reelPointer(state: FishingState): number {
  const position = (state.reelElapsed * 0.73) % 2;
  return position <= 1 ? position : 2 - position;
}

export function reelTarget(state: FishingState): { start: number; width: number } {
  const center = [0.32, 0.73, 0.3][Math.min(state.reelHits, 2)]!;
  const width = 0.3 - (state.fish?.species ?? 0) * 0.018;
  return { start: center - width / 2, width };
}

export function advanceFishing(state: FishingState, dt: number): boolean {
  if (!Number.isFinite(dt) || dt <= 0) return false;
  const step = Math.min(dt, 1);
  if (state.phase === "waiting") {
    state.elapsed += step;
    if (state.elapsed >= state.waitSeconds) { state.phase = "bite"; state.elapsed = 0; return true; }
  } else if (state.phase === "bite") {
    state.elapsed += step;
    if (state.elapsed >= 2.4) { state.phase = "escaped"; state.escapeReason = "late"; return true; }
  } else if (state.phase === "reeling") {
    state.reelElapsed += step;
    if (state.reelElapsed >= 9) { state.phase = "escaped"; state.escapeReason = "late"; return true; }
  }
  return false;
}

export function actFishing(state: FishingState, random = Math.random): boolean {
  if (["idle", "caught", "escaped"].includes(state.phase)) {
    const roll = Math.max(0, Math.min(0.999999, random()));
    const species = roll < 0.38 ? 0 : roll < 0.66 ? 1 : roll < 0.84 ? 2 : roll < 0.95 ? 3 : 4;
    const definition = FISH_SPECIES[species]!;
    const weight = Math.round((definition.min + Math.max(0, Math.min(1, random())) * (definition.max - definition.min)) * 100) / 100;
    Object.assign(state, { phase: "waiting", elapsed: 0, waitSeconds: 1.8 + Math.max(0, Math.min(1, random())) * 2.4, fish: { species, weight }, reelElapsed: 0, reelHits: 0, lastPull: -1, lastPoints: 0, escapeReason: "" });
    return true;
  }
  if (state.phase === "bite") { state.phase = "reeling"; state.elapsed = 0; return true; }
  if (state.phase !== "reeling" || state.reelElapsed - state.lastPull < 0.2) return false;
  const target = reelTarget(state);
  const position = reelPointer(state);
  state.lastPull = state.reelElapsed;
  if (position < target.start || position > target.start + target.width) {
    state.phase = "escaped"; state.escapeReason = "miss"; return true;
  }
  state.reelHits += 1;
  if (state.reelHits === 3 && state.fish) {
    state.phase = "caught";
    state.lastPoints = fishPoints(state.fish);
    state.score += state.lastPoints;
    state.caught += 1;
    state.collection[state.fish.species]! += 1;
  }
  return true;
}
