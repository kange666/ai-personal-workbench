import { onBeforeUnmount, onMounted, watch } from "vue";

/** 暂停、隐藏和卸载时取消帧循环，恢复时不补算离开期间的时间。 */
export function useGameLoop(paused: () => boolean, tick: (seconds: number) => void, options: { pauseOnBlur?: boolean } = {}) {
  // 默认失焦暂停；放置类游戏可继续运行，隐藏窗口和手动暂停仍停止循环。
  const pauseOnBlur = options.pauseOnBlur !== false;
  let frame = 0;
  let lastTime = 0;
  let mounted = false;
  let focused = true;
  function stop() { cancelAnimationFrame(frame); frame = 0; lastTime = 0; }
  function run(now: number) {
    frame = 0;
    if (!mounted || paused() || document.hidden || !focused) { lastTime = 0; return; }
    if (lastTime) tick(Math.min(Math.max((now - lastTime) / 1000, 0), 0.05));
    lastTime = now;
    frame = requestAnimationFrame(run);
  }
  function sync() {
    stop();
    if (mounted && !paused() && !document.hidden && focused) frame = requestAnimationFrame(run);
  }
  const blur = () => { focused = false; stop(); };
  const focus = () => { focused = true; sync(); };
  watch(paused, sync);
  onMounted(() => {
    mounted = true;
    if (pauseOnBlur) {
      window.addEventListener("blur", blur);
      window.addEventListener("focus", focus);
    }
    document.addEventListener("visibilitychange", sync);
    sync();
  });
  onBeforeUnmount(() => {
    mounted = false; stop();
    if (pauseOnBlur) {
      window.removeEventListener("blur", blur);
      window.removeEventListener("focus", focus);
    }
    document.removeEventListener("visibilitychange", sync);
  });
}
