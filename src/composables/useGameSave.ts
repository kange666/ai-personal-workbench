import { onBeforeUnmount, onMounted, ref, type Ref } from "vue";

export const restStorageWarning = ref("");
export const gameSaveKey = (id: string) => `astrion-rest-v1:${id}`;
// 覆盖堆塔允许的 10,000 层；读写使用相同上限，超限时保留上一份存档。
const MAX_SAVE_CHARS = 1_000_000;
const scoreValue = (value: unknown) => typeof value === "number" && Number.isFinite(value) && value >= 0 ? Math.floor(value) : 0;

function readEnvelope(id: string): { state?: unknown; best?: unknown; version?: unknown } | null {
  try {
    const raw = localStorage.getItem(gameSaveKey(id));
    if (!raw) return null;
    if (raw.length > MAX_SAVE_CHARS) throw new Error("oversize");
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error("invalid");
    return parsed;
  } catch {
    restStorageWarning.value = "无法读取部分游戏存档，将从新的一局开始。";
    return null;
  }
}
export function gameBest(id: string) { return scoreValue(readEnvelope(id)?.best); }

/** 每款游戏独立存档。逐回合或最多每秒保存，避免在动画每帧写入同步存储。 */
export function useGameSave<T>(id: string, create: () => T, validate: (value: unknown) => value is T, migrate?: (value: unknown) => T | null) {
  const stored = readEnvelope(id);
  let valid = false;
  let restored = stored?.state;
  try {
    if (stored?.version === 1) {
      if (!validate(restored) && migrate) restored = migrate(restored);
      valid = validate(restored);
    }
  } catch { /* 损坏存档由新局替代。 */ }
  if (stored && !valid) restStorageWarning.value = "这款游戏的进度不可用，已新开一局并保留有效最高分。";
  const state = ref(valid ? restored as T : create()) as Ref<T>;
  const best = ref(scoreValue(stored?.best));
  const storageError = ref(false);
  function save() {
    try {
      best.value = Math.max(best.value, gameBest(id));
      const serialized = JSON.stringify({ version: 1, state: state.value, best: best.value });
      if (serialized.length > MAX_SAVE_CHARS) throw new Error("oversize");
      localStorage.setItem(gameSaveKey(id), serialized);
      if (storageError.value) restStorageWarning.value = "";
      storageError.value = false;
    } catch {
      storageError.value = true;
      restStorageWarning.value = "本地存储暂时不可用，当前仍可游玩，但进度可能无法保存。";
    }
  }
  function recordScore(score: number) { best.value = Math.max(best.value, scoreValue(score)); save(); }
  function restart() { state.value = create(); save(); }
  const saveWhenHidden = () => { if (document.hidden) save(); };
  onMounted(() => {
    window.addEventListener("pagehide", save);
    window.addEventListener("blur", save);
    document.addEventListener("visibilitychange", saveWhenHidden);
  });
  onBeforeUnmount(() => {
    save();
    window.removeEventListener("pagehide", save);
    window.removeEventListener("blur", save);
    document.removeEventListener("visibilitychange", saveWhenHidden);
  });
  return { state, best, save, recordScore, restart, storageError };
}
