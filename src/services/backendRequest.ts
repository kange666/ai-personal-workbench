import { invoke as tauriInvoke } from "@tauri-apps/api/core";

export const backendLoadingEvent = "workbench-backend-loading";
let currentScope = 0;
let backgroundDepth = 0;
const activeByScope = new Map<number, number>();

function emitLoading(command = "") {
  if (typeof window !== "undefined") window.dispatchEvent(new CustomEvent(backendLoadingEvent, {
    detail: { active: activeByScope.get(currentScope) || 0, scope: currentScope, command },
  }));
}

/** 新页面只等待自己发起的请求，不继承上一个页面的慢请求。 */
export function beginBackendPageScope() {
  currentScope += 1;
  emitLoading();
}

/** 仅包裹一个命令的同步发起阶段，不能包裹含多个 await 的加载流程。 */
export function backgroundRequest<T>(request: () => Promise<T>): Promise<T> {
  backgroundDepth += 1;
  try { return request(); }
  finally { backgroundDepth -= 1; }
}

export async function invoke<T = void>(command: string, args?: Record<string, unknown>): Promise<T> {
  const scope = currentScope;
  const foreground = backgroundDepth === 0;
  if (foreground) {
    activeByScope.set(scope, (activeByScope.get(scope) || 0) + 1);
    emitLoading(command);
  }
  try { return await tauriInvoke<T>(command, args); }
  finally {
    if (foreground) {
      const active = Math.max(0, (activeByScope.get(scope) || 0) - 1);
      if (active) activeByScope.set(scope, active); else activeByScope.delete(scope);
      emitLoading(command);
    }
  }
}
