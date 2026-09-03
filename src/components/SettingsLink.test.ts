import { flushPromises, mount, RouterLinkStub } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { checkForUpdates, isTauriRuntime, type UpdateStatus } from "../services/backend";
import { latestUpdateStatus, refreshUpdateStatus } from "../services/updateStatus";
import SettingsLink from "./SettingsLink.vue";

vi.mock("../services/backend", () => ({ checkForUpdates: vi.fn(), isTauriRuntime: vi.fn() }));

const response = (available: boolean, version = "1.9.5"): UpdateStatus => ({
  currentVersion: "1.9.4", latestVersion: version, updateAvailable: available,
  publishedAt: "", releaseUrl: "", installerUrl: "", portableUrl: "", checkedAt: "", message: "",
});
const wrappers: ReturnType<typeof mount>[] = [];
function renderLink() {
  const wrapper = mount(SettingsLink, { global: { stubs: { RouterLink: RouterLinkStub } } });
  wrappers.push(wrapper);
  return wrapper;
}

beforeEach(async () => {
  vi.useFakeTimers();
  vi.mocked(isTauriRuntime).mockReturnValue(true);
  vi.mocked(checkForUpdates).mockResolvedValue(response(false));
  await refreshUpdateStatus();
  vi.mocked(checkForUpdates).mockClear();
});
afterEach(() => {
  wrappers.splice(0).forEach(wrapper => wrapper.unmount());
  vi.useRealTimers();
});

describe("设置菜单新版本提示", () => {
  it("等待查询和没有新版本时不显示图标", async () => {
    const wrapper = renderLink();
    expect(wrapper.find(".settings-update-badge").exists()).toBe(false);
    await flushPromises();
    expect(wrapper.find(".settings-update-badge").exists()).toBe(false);
    expect(wrapper.attributes("aria-label")).toBe("设置");
  });

  it("发现新版本显示小图标及版本说明，保持进入设置页", async () => {
    vi.mocked(checkForUpdates).mockResolvedValue(response(true, "V1.9.5"));
    const wrapper = renderLink();
    await flushPromises();
    expect(wrapper.find(".settings-update-badge").exists()).toBe(true);
    expect(wrapper.attributes("title")).toContain("发现新版本 V1.9.5");
    expect(wrapper.attributes("aria-label")).toContain("点击查看更新");
    expect(wrapper.getComponent(RouterLinkStub).props("to")).toBe("/settings");
  });

  it("设置页再次检查的结果实时同步到菜单，并能清除提示", async () => {
    const wrapper = renderLink();
    await flushPromises();
    vi.mocked(checkForUpdates).mockResolvedValue(response(true));
    await refreshUpdateStatus();
    await flushPromises();
    expect(wrapper.find(".settings-update-badge").exists()).toBe(true);
    vi.mocked(checkForUpdates).mockResolvedValue(response(false));
    await refreshUpdateStatus();
    await flushPromises();
    expect(wrapper.find(".settings-update-badge").exists()).toBe(false);
  });

  it("后台每小时复查，组件卸载后不再检查", async () => {
    const wrapper = renderLink();
    await flushPromises();
    expect(checkForUpdates).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(60 * 60 * 1000);
    expect(checkForUpdates).toHaveBeenCalledTimes(2);
    wrapper.unmount();
    await vi.advanceTimersByTimeAsync(60 * 60 * 1000);
    expect(checkForUpdates).toHaveBeenCalledTimes(2);
  });

  it("浏览器预览不调用桌面更新接口", async () => {
    vi.mocked(isTauriRuntime).mockReturnValue(false);
    renderLink();
    await vi.advanceTimersByTimeAsync(60 * 60 * 1000);
    expect(checkForUpdates).not.toHaveBeenCalled();
  });

  it("网络失败不误报新版本，也不清除已经确认的提示", async () => {
    vi.mocked(checkForUpdates).mockResolvedValue(response(false, ""));
    await refreshUpdateStatus();
    expect(latestUpdateStatus.value?.updateAvailable).toBe(false);
    vi.mocked(checkForUpdates).mockResolvedValue(response(true));
    await refreshUpdateStatus();
    vi.mocked(checkForUpdates).mockResolvedValue(response(false, ""));
    await refreshUpdateStatus();
    expect(latestUpdateStatus.value?.updateAvailable).toBe(true);
  });

  it("后台接口异常保持静默，下一次检查仍可恢复", async () => {
    vi.mocked(checkForUpdates).mockRejectedValueOnce(new Error("offline"));
    const wrapper = renderLink();
    await flushPromises();
    expect(wrapper.find(".settings-update-badge").exists()).toBe(false);
    vi.mocked(checkForUpdates).mockResolvedValue(response(true));
    await vi.advanceTimersByTimeAsync(60 * 60 * 1000);
    expect(wrapper.find(".settings-update-badge").exists()).toBe(true);
  });

  it("菜单和设置同时检查时复用同一个请求", async () => {
    let resolve!: (status: UpdateStatus) => void;
    vi.mocked(checkForUpdates).mockReturnValueOnce(new Promise(done => { resolve = done; }));
    const first = refreshUpdateStatus();
    const second = refreshUpdateStatus();
    expect(first).toBe(second);
    expect(checkForUpdates).toHaveBeenCalledTimes(1);
    resolve(response(true));
    await first;
  });
});
