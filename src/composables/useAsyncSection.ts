import { reactive } from "vue";

export interface AsyncSectionState { loading: boolean; error: string; updatedAt: string }

/** 每张卡片独立提交成功数据；失败不清空旧值，重复刷新合并成一轮。 */
export function useAsyncSection(read: (refresh: boolean) => Promise<void>) {
  const state = reactive<AsyncSectionState>({ loading: false, error: "", updatedAt: "" });
  let pending: Promise<void> | undefined;
  let refreshAgain = false;
  function load(refresh = false): Promise<void> {
    if (pending) { refreshAgain ||= refresh; return pending; }
    state.loading = true; state.error = "";
    pending = (async () => {
      let rebuild = refresh;
      do {
        refreshAgain = false;
        try { await read(rebuild); state.updatedAt = new Date().toISOString(); state.error = ""; }
        catch (cause) { state.error = `读取失败：${String(cause)}`; }
        rebuild = refreshAgain;
      } while (refreshAgain);
    })().finally(() => { state.loading = false; pending = undefined; });
    return pending;
  }
  return { state, load };
}
