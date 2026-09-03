const cacheStorageKey = "ai-workbench.testing.menu-cache.v1";
const cacheMaxAge = 30 * 60 * 1000;
const cacheProjectLimit = 4;

type MenuCacheEntry<T> = { savedAt: number; menus: T[] };
type MenuCache<T> = Record<string, MenuCacheEntry<T>>;

function projectKey(value: string) {
  return value.replace(/^\\\\\?\\/, "").replace(/\//g, "\\").replace(/\\+$/, "").toLowerCase();
}

export function readTestingMenuCache<T>(storage: Storage, projectPath: string, now = Date.now()): T[] {
  try {
    const cache = JSON.parse(storage.getItem(cacheStorageKey) || "{}") as MenuCache<T>;
    const entry = cache[projectKey(projectPath)];
    return entry && now - entry.savedAt < cacheMaxAge && Array.isArray(entry.menus) ? entry.menus : [];
  } catch {
    return [];
  }
}

export function writeTestingMenuCache<T>(storage: Storage, projectPath: string, menus: T[], now = Date.now()) {
  try {
    const cache = JSON.parse(storage.getItem(cacheStorageKey) || "{}") as MenuCache<T>;
    cache[projectKey(projectPath)] = { savedAt: now, menus };
    const recent = Object.fromEntries(
      Object.entries(cache)
        .sort((left, right) => right[1].savedAt - left[1].savedAt)
        .slice(0, cacheProjectLimit),
    );
    storage.setItem(cacheStorageKey, JSON.stringify(recent));
  } catch {
    // 会话缓存不可用时保持静默，真实菜单读取仍可继续。
  }
}
