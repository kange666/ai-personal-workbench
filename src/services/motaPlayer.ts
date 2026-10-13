export interface MotaPlayerApi {
  load(options: Record<string, unknown>): Promise<void>;
  suspend(): void;
  resume(): void;
}
export interface MotaPlayer extends HTMLElement { ruffle(): MotaPlayerApi }
interface RuffleSource { createPlayer(): MotaPlayer }
interface RuffleRuntime { newest(): RuffleSource }

const runtimePath = "/rest/mota21/ruffle/ruffle.js";
export const motaMoviePath = "/rest/mota21/classic-21.swf";
let runtimeLoading: Promise<RuffleRuntime> | null = null;
let playerLoading: Promise<MotaPlayer> | null = null;
let playerHost: HTMLDivElement | null = null;
let generation = 0;

function loadRuntime(): Promise<RuffleRuntime> {
  if (runtimeLoading) return runtimeLoading;
  runtimeLoading = new Promise((resolve, reject) => {
    const script = document.createElement("script");
    script.src = runtimePath;
    script.addEventListener("load", () => {
      const runtime = (window as Window & { RufflePlayer?: RuffleRuntime }).RufflePlayer;
      if (runtime?.newest) resolve(runtime);
      else { runtimeLoading = null; script.remove(); reject(new Error("播放器初始化失败")); }
    }, { once: true });
    script.addEventListener("error", () => {
      runtimeLoading = null; script.remove(); reject(new Error("播放器加载失败"));
    }, { once: true });
    document.head.appendChild(script);
  });
  return runtimeLoading;
}

/** 原版没有持久存盘：复用同一播放器，关闭弹窗后保留本次运行进度。 */
export function getMotaPlayer(): Promise<MotaPlayer> {
  if (!playerLoading) {
    const current = ++generation;
    playerLoading = loadRuntime().then(async runtime => {
      if (current !== generation) throw new Error("此次加载已取消");
      const player = runtime.newest().createPlayer();
      const host = document.createElement("div");
      host.className = "mota-player-host";
      Object.assign(host.style, { position: "fixed", zIndex: "1501", display: "none", width: "570px", height: "410px", overflow: "hidden", borderRadius: "8px" });
      playerHost = host;
      player.className = "mota-player";
      Object.assign(player.style, { display: "block", width: "100%", height: "100%" });
      player.tabIndex = 0;
      player.setAttribute("aria-label", "经典 21 层魔塔，方向键移动，空格确认对话");
      // Ruffle 离开文档时会销毁运行状态，整个会话中保持播放器节点连接。
      host.appendChild(player);
      document.body.appendChild(host);
      // 游戏与运行库都来自本地；禁用原 Flash 的外链和脚本调用。
      try {
        await player.ruffle().load({
          url: motaMoviePath, autoplay: "on", scale: "showAll", letterbox: "on",
          allowScriptAccess: false, allowNetworking: "none", openUrlMode: "deny",
          unmuteOverlay: "hidden", contextMenu: "off", showSwfDownload: false,
        });
      } catch (error) { host.remove(); if (playerHost === host) playerHost = null; throw error; }
      player.ruffle().suspend();
      return player;
    }).catch(error => { if (current === generation) playerLoading = null; throw error; });
  }
  return playerLoading;
}

/** 只重置本次魔塔，不修改其他小游戏的存档。 */
export function discardMotaPlayer() { generation++; playerHost?.remove(); playerHost = null; playerLoading = null; }

type MotaRect = Pick<DOMRect, "left" | "top" | "width" | "height">;
export function positionMotaPlayer(rect: MotaRect, bounds: MotaRect = rect) {
  if (!playerHost) return;
  // 持久节点不在弹窗的滚动容器内，需要主动裁切，避免画面盖住弹窗外的内容。
  const top = Math.max(0, bounds.top - rect.top);
  const right = Math.max(0, rect.left + rect.width - bounds.left - bounds.width);
  const bottom = Math.max(0, rect.top + rect.height - bounds.top - bounds.height);
  const left = Math.max(0, bounds.left - rect.left);
  if (left + right >= rect.width || top + bottom >= rect.height) { hideMotaPlayer(); return; }
  Object.assign(playerHost.style, {
    left: `${rect.left}px`, top: `${rect.top}px`, width: `${rect.width}px`, height: `${rect.height}px`,
    clipPath: `inset(${top}px ${right}px ${bottom}px ${left}px)`, display: "block",
  });
  playerHost.inert = false;
}
export function hideMotaPlayer() {
  if (playerHost) { playerHost.style.display = "none"; playerHost.inert = true; }
}
