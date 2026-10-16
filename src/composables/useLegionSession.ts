import { shallowRef } from 'vue';
import { gameSaveKey, restStorageWarning } from './useGameSave';
import { createLegion, restartLegion, stepLegion, validLegion, type LegionState } from '../utils/rest/legion';

let session: ReturnType<typeof createSession> | undefined;
function createSession() {
  let initial = createLegion();
  try {
    const raw = localStorage.getItem(gameSaveKey('legion'));
    if (raw) { const envelope = raw.length <= 1_000_000 ? JSON.parse(raw) : null;
      if (envelope?.version === 1 && validLegion(envelope.state)) initial = envelope.state;
      else restStorageWarning.value = '军团存档不可用，已重新开始远征。其他游戏存档不受影响。';
    }
  } catch { restStorageWarning.value = '暂时无法读取军团存档，当前仍可开始远征。'; }
  const state = shallowRef<LegionState>(initial);
  let viewers = 0, saved = 0, timer: ReturnType<typeof setTimeout>, last = performance.now(), storageFailed = false;
  function save() {
    try { localStorage.setItem(gameSaveKey('legion'), JSON.stringify({ version: 1, state: state.value, best: state.value.bestRun }));
      if (storageFailed) restStorageWarning.value = ''; storageFailed = false;
    } catch { storageFailed = true; restStorageWarning.value = '本地存储暂时不可用，军团仍在运行，但进度可能无法保存。'; }
  }
  // 发布浅快照，让战场子组件也能收到变化；规则对象无需深层代理。
  function publish() { state.value = { ...state.value }; }
  function mutate(action: (s: LegionState) => void) { action(state.value); publish(); save(); }
  function pulse() {
    const now = performance.now(), seconds = Math.min(5, Math.max(0, (now - last) / 1000)); last = now;
    // 前台20次/秒绘图，收起后每秒2次结算；复用同一战斗规则，不制造离线收益。
    let remaining = seconds * state.value.speed;
    while (remaining > .00001) { const dt = Math.min(.1, remaining); stepLegion(state.value, dt); remaining -= dt; }
    if (viewers && !document.hidden) publish();
    saved += seconds; if (saved >= 2) { save(); saved = 0; }
    timer = setTimeout(pulse, viewers && !document.hidden ? 50 : 500);
  }
  const pagehide = () => save(); window.addEventListener('pagehide', pagehide);
  timer = setTimeout(pulse, 50); save();
  return { state, save, mutate,
    attach() { viewers++; publish(); return () => { viewers = Math.max(0, viewers - 1); save(); }; },
    restart() { state.value = restartLegion(state.value); save(); },
    dispose() { clearTimeout(timer); window.removeEventListener('pagehide', pagehide); save(); },
  };
}
export function useLegionSession() { return session ??= createSession(); }
// 仅供热更新和测试释放当前模块创建的计时器，不触碰其他游戏或应用进程。
export function disposeLegionSession() { session?.dispose(); session = undefined; }
if (import.meta.hot) import.meta.hot.dispose(disposeLegionSession);
