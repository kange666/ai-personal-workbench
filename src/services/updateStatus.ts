import { readonly, shallowRef } from "vue";
import { checkForUpdates, type UpdateStatus } from "./backend";

const status = shallowRef<UpdateStatus | null>(null);
export const latestUpdateStatus = readonly(status);
let pendingCheck: Promise<UpdateStatus> | undefined;

/** 菜单提示和设置页共享检查结果；同时进入时只发出一次请求。 */
export function refreshUpdateStatus(): Promise<UpdateStatus> {
  if (!pendingCheck) {
    pendingCheck = checkForUpdates().then(result => {
      // 网络失败时后端返回空版本号，不能据此清除已经确认的新版本提示。
      if (result.latestVersion.trim()) status.value = result;
      return result;
    }).finally(() => { pendingCheck = undefined; });
  }
  return pendingCheck;
}
