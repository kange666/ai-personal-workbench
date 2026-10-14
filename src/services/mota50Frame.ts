/** 50 层独立运行在本地页面中；节点持续连接，关闭弹窗时只暂停。 */
let frame: HTMLIFrameElement | null = null;
let host: HTMLDivElement | null = null;
export const mota50Channel = "astrion-mota50-v1";

export function getMota50Frame() {
  if (!frame) {
    host = document.createElement("div");
    host.className = "mota-player-host mota50-player-host";
    Object.assign(host.style, { position: "fixed", zIndex: "1501", display: "none", overflow: "hidden", borderRadius: "8px" });
    host.inert = true;
    frame = document.createElement("iframe");
    frame.src = "/rest/mota50/index.html";
    frame.title = "经典 50 层魔塔";
    // 原生存档使用本机存储；禁止弹出窗口、顶层导航和外部表单提交。
    frame.setAttribute("sandbox", "allow-scripts allow-same-origin allow-modals allow-downloads");
    Object.assign(frame.style, { display: "block", width: "100%", height: "100%", border: "0", background: "#000" });
    host.appendChild(frame);
    document.body.appendChild(host);
  }
  return frame;
}

export function commandMota50(type: "status" | "pause" | "resume" | "save" | "load") {
  frame?.contentWindow?.postMessage({ channel: mota50Channel, type }, window.location.origin);
}

type Rect = Pick<DOMRect, "left" | "top" | "width" | "height">;
export function positionMota50Frame(rect: Rect, bounds: Rect = rect) {
  if (!host) return;
  const top = Math.max(0, bounds.top - rect.top), left = Math.max(0, bounds.left - rect.left);
  const right = Math.max(0, rect.left + rect.width - bounds.left - bounds.width);
  const bottom = Math.max(0, rect.top + rect.height - bounds.top - bounds.height);
  if (left + right >= rect.width || top + bottom >= rect.height) { hideMota50Frame(); return; }
  Object.assign(host.style, { left: `${rect.left}px`, top: `${rect.top}px`, width: `${rect.width}px`, height: `${rect.height}px`, clipPath: `inset(${top}px ${right}px ${bottom}px ${left}px)`, display: "block" });
  host.inert = false;
}

export function hideMota50Frame() {
  commandMota50("pause");
  if (host) { host.style.display = "none"; host.inert = true; }
}

/** 重开仅重建运行实例；已有手动存档保留。 */
export function discardMota50Frame() {
  hideMota50Frame(); host?.remove(); host = null; frame = null;
}
