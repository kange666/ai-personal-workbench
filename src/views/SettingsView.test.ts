import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as backend from "../services/backend";
import SettingsView from "./SettingsView.vue";
import { applyFontSize, loadFontSize } from "../utils/fontSize";
import { relaunch } from "@tauri-apps/plugin-process";
import { confirmAction } from "../utils/confirm";

vi.mock("../services/backend");
vi.mock("../services/updateStatus", () => ({ refreshUpdateStatus: vi.fn(async () => ({})) }));
vi.mock("@tauri-apps/plugin-autostart", () => ({ isEnabled: vi.fn(async () => false) }));
vi.mock("vue-router", () => ({ useRoute: () => ({ query: {} }) }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: vi.fn(async () => {}) }));
vi.mock("../utils/confirm", () => ({ confirmAction: vi.fn(async () => true) }));

let wrapper: VueWrapper;
function mockLocalSettings() {
  vi.mocked(backend.isTauriRuntime).mockReturnValue(true);
  vi.mocked(backend.getAiStatus).mockResolvedValue({ configured: false, source: "未配置", model: "demo" });
  vi.mocked(backend.getApifoxCredentialStatus).mockResolvedValue({ configured: false, source: "未配置" });
  vi.mocked(backend.getTapdStatus).mockResolvedValue({ authMode: "token", owner: "", projects: [] } as unknown as backend.TapdStatus);
  vi.mocked(backend.getEmailNotificationStatus).mockResolvedValue({ state: "unconfigured" } as backend.EmailNotificationStatus);
  vi.mocked(backend.getVipStatus).mockResolvedValue({ active: false });
  vi.mocked(backend.getTrayIconStyle).mockResolvedValue("B");
  vi.mocked(backend.getWorkTimeSettings).mockResolvedValue({ gapMinutes: 45 });
  vi.mocked(backend.getBackupStatus).mockResolvedValue({ databasePath: "anonymous.sqlite", backupDirectory: "backups", backups: [
    { fileName: "old.sqlite", path: "backups/old.sqlite", kind: "manual", createdAt: "2026-09-29T01:00:00Z", sizeBytes: 1024 },
  ] });
}
async function renderView() {
  wrapper = mount(SettingsView, {
    global: { stubs: { ThemeSwitch: true, NavIcon: true, RouterLink: true } },
  });
  await flushPromises();
  return wrapper;
}

beforeEach(() => {
  localStorage.clear();
  applyFontSize("medium");
  vi.resetAllMocks();
  vi.mocked(backend.isTauriRuntime).mockReturnValue(false);
  vi.mocked(backend.setTrayIconStyle).mockImplementation(async (style) => style);
});
afterEach(() => { wrapper?.unmount(); vi.restoreAllMocks(); applyFontSize("medium"); localStorage.clear(); });

describe("外观中的页面字号", () => {
  it("与主题和托盘归在外观中，并保留独立的工时设置", async () => {
    await renderView();
    const appearance = wrapper.get(".appearance-settings");
    const card = wrapper.get(".worktime-settings");
    expect(card.get('input[type="number"]').element).toHaveProperty("value", "45");
    expect(card.find(".font-size-options").exists()).toBe(false);
    expect(appearance.findAll('.font-size-options label').map((label) => label.text())).toEqual(["小", "中", "大", "超大"]);
    expect(appearance.get('input[value="medium"]').element).toHaveProperty("checked", true);
  });
  it("切换即时应用且不提交工时设置，重进页面保留选择", async () => {
    await renderView();
    await wrapper.get('input[value="extra-large"]').setValue();
    expect(loadFontSize()).toBe("extra-large");
    expect(document.documentElement.style.getPropertyValue("--app-font-offset")).toBe("4px");
    expect(backend.saveWorkTimeSettings).not.toHaveBeenCalled();
    wrapper.unmount();
    await renderView();
    expect(wrapper.get('input[value="extra-large"]').element).toHaveProperty("checked", true);
  });
  it("字号无法保存时提示错误，不改变已应用的字号", async () => {
    await renderView();
    const save = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("denied"); });
    await wrapper.get('input[value="large"]').setValue();
    expect(wrapper.get(".scan-message.error").text()).toContain("字号设置保存失败");
    expect(document.documentElement.dataset.fontSize).toBe("medium");
    expect(wrapper.get('input[value="medium"]').element).toHaveProperty("checked", true);
    expect(wrapper.get('input[value="large"]').element).toHaveProperty("checked", false);
    save.mockRestore();
  });
});

describe("设置分类与工作台消息", () => {
  it("把数据、VIP 和更新归为最后一个分类", async () => {
    await renderView();
    const categories = wrapper.findAll(".settings-navigation nav button").map((button) => button.text());
    expect(categories).toHaveLength(6);
    expect(categories.at(-1)).toContain("功能与数据");
  });

  it("工作台消息默认开启，关闭后保存本机偏好", async () => {
    await renderView();
    await wrapper.findAll(".settings-navigation nav button")[4].trigger("click");
    const toggle = wrapper.get('.workbench-notification-settings [role="switch"]');
    expect(toggle.attributes("aria-checked")).toBe("true");
    await toggle.trigger("click");
    expect(toggle.attributes("aria-checked")).toBe("false");
    expect(localStorage.getItem("workbench-notifications-enabled")).toBe("false");
    expect(wrapper.get(".scan-message").text()).toContain("工作台消息通知已关闭");
  });
});

describe("本地恢复与独立设置读取", () => {
  async function openData() {
    await renderView();
    await wrapper.findAll(".settings-navigation nav button")[5].trigger("click");
    return wrapper.get(".data-safety-collapsible .danger-text");
  }

  it("外部服务读取失败不影响本地备份和字号设置", async () => {
    mockLocalSettings();
    vi.mocked(backend.getTapdStatus).mockRejectedValue(new Error("unavailable"));
    await openData();
    expect(wrapper.get(".backup-list").text()).toContain("手工");
    expect(wrapper.get(".scan-message.error").text()).toContain("TAPD设置读取失败");
    expect(backend.getWorkTimeSettings).toHaveBeenCalledOnce();
  });

  it("恢复成功后真正重启，不再查询旧运行期数据库", async () => {
    mockLocalSettings();
    vi.mocked(backend.restoreDatabaseBackup).mockResolvedValue({ databasePath: "anonymous.sqlite", backupDirectory: "backups", backups: [] });
    const restore = await openData();
    const initialReads = vi.mocked(backend.getBackupStatus).mock.calls.length;
    await restore.trigger("click"); await flushPromises();
    expect(backend.restoreDatabaseBackup).toHaveBeenCalledWith("backups/old.sqlite");
    expect(relaunch).toHaveBeenCalledOnce();
    expect(backend.getBackupStatus).toHaveBeenCalledTimes(initialReads);
    expect(wrapper.text()).toContain("正在重新启动工作台");
  });

  it("恢复失败保留旧数据且不重启，允许再次操作", async () => {
    mockLocalSettings();
    vi.mocked(backend.restoreDatabaseBackup).mockRejectedValue(new Error("备份校验失败"));
    await (await openData()).trigger("click"); await flushPromises();
    expect(relaunch).not.toHaveBeenCalled();
    expect(wrapper.get(".scan-message.error").text()).toContain("备份校验失败");
    expect(wrapper.get(".data-safety-collapsible .danger-text").attributes("disabled")).toBeUndefined();
  });

  it("确认期间不重复恢复，重启失败给出明确提示并保持数据操作锁定", async () => {
    mockLocalSettings();
    let confirm!: (value:boolean)=>void;
    vi.mocked(confirmAction).mockReturnValue(new Promise(resolve=>{confirm=resolve;}));
    vi.mocked(backend.restoreDatabaseBackup).mockResolvedValue({ databasePath: "anonymous.sqlite", backupDirectory: "backups", backups: [] });
    vi.mocked(relaunch).mockRejectedValue(new Error("restart blocked"));
    const restore=await openData();
    await restore.trigger("click"); await restore.trigger("click");
    expect(confirmAction).toHaveBeenCalledOnce();
    confirm(true); await flushPromises();
    expect(backend.restoreDatabaseBackup).toHaveBeenCalledOnce();
    expect(wrapper.get(".scan-message.error").text()).toContain("数据已安全恢复，但自动重启失败");
    expect(wrapper.get(".data-safety-body .button.primary").attributes("disabled")).toBeDefined();
  });

  it("诊断按需读取，失败后可以重试", async () => {
    mockLocalSettings();
    vi.mocked(backend.listBackgroundTaskStatus).mockRejectedValueOnce(new Error("offline")).mockResolvedValueOnce([]);
    await openData();
    expect(backend.listBackgroundTaskStatus).not.toHaveBeenCalled();
    const details=wrapper.get(".diagnostic-settings");
    (details.element as HTMLDetailsElement).open=true;
    await vi.waitFor(()=>expect(details.text()).toContain("运行诊断读取失败"));
    await details.get("button").trigger("click"); await flushPromises();
    expect(details.text()).toContain("本次启动尚无后台任务记录");
  });
});

describe("自动化中的驾驶舱屏保", () => {
  it("默认十分钟，切换为永不自动进入后立即保存", async () => {
    await renderView();
    const card = wrapper.get(".cockpit-automation-settings");
    const select = card.get("select");
    expect((select.element as HTMLSelectElement).value).toBe("10");
    await select.setValue("never");
    expect(localStorage.getItem("workbench-cockpit-idle-minutes-v1")).toBe("never");
    expect(card.text()).toContain("已关闭自动进入");
    expect(wrapper.get(".scan-message").text()).toContain("驾驶舱屏保已关闭");
  });

  it("切换分钟数后立即更新本机偏好", async () => {
    await renderView();
    await wrapper.get(".cockpit-automation-settings select").setValue("30");
    expect(localStorage.getItem("workbench-cockpit-idle-minutes-v1")).toBe("30");
    expect(wrapper.get(".cockpit-automation-settings").text()).toContain("30 分钟后进入");
  });
});

describe("外观与托盘风格", () => {
  it("显示八种有名称的样式，进度预览不含数字，移除冗余说明", async () => {
    await renderView();
    const appearance = wrapper.get(".appearance-settings");
    expect(appearance.findAll('[role="radio"]').map((option) => option.text()))
      .toEqual(["57紫底", "57白底", "57荧光", "57纯数字", "57浅色数字", "圆环", "电量条", "分段柱"]);
    expect(appearance.text()).toContain("托盘风格");
    expect(appearance.text()).not.toMatch(/默认 B|立即生效|托盘数字风格/);
    for (const style of ["c", "d", "e"]) {
      expect(appearance.get(`.style-${style}`).find("svg").exists()).toBe(true);
      expect(appearance.get(`.style-${style}`).find("i").exists()).toBe(false);
    }
  });

  it.each(["G", "H", "D", "E"] as const)("切换 %s 后保存新样式并更新选中状态", async (style) => {
    await renderView();
    await wrapper.get(`.style-${style.toLowerCase()}`).trigger("click");
    await flushPromises();
    expect(backend.setTrayIconStyle).toHaveBeenCalledWith(style);
    expect(wrapper.get(`.style-${style.toLowerCase()}`).attributes("aria-checked")).toBe("true");
    expect(wrapper.get(".style-b").attributes("aria-checked")).toBe("false");
    expect(wrapper.find(".scan-message").exists()).toBe(false);
  });

  it("保存期间禁用切换，失败时保持原样式并显示错误", async () => {
    let rejectSave!: (cause: Error) => void;
    vi.mocked(backend.setTrayIconStyle).mockReturnValue(new Promise((_, reject) => { rejectSave = reject; }));
    await renderView();
    await wrapper.get(".style-g").trigger("click");
    expect(wrapper.findAll('[role="radio"]').every((option) => option.attributes("disabled") !== undefined)).toBe(true);
    expect(wrapper.get(".style-b").attributes("aria-checked")).toBe("true");
    rejectSave(new Error("保存失败"));
    await flushPromises();
    expect(wrapper.get(".style-b").attributes("aria-checked")).toBe("true");
    expect(wrapper.get(".scan-message.error").text()).toContain("保存失败");
    expect(wrapper.get(".style-g").attributes("disabled")).toBeUndefined();
  });

  it.each(["G", "H"] as const)("重新进入页面恢复已保存的 %s 纯数字风格", async (style) => {
    vi.mocked(backend.isTauriRuntime).mockReturnValue(true);
    vi.mocked(backend.getAiStatus).mockResolvedValue({ configured: false, source: "未配置", model: "demo" });
    vi.mocked(backend.getApifoxCredentialStatus).mockResolvedValue({ configured: false, source: "未配置" });
    vi.mocked(backend.getTapdStatus).mockResolvedValue({ authMode: "token", owner: "", projects: [] } as unknown as backend.TapdStatus);
    vi.mocked(backend.getEmailNotificationStatus).mockResolvedValue({ state: "unconfigured" } as backend.EmailNotificationStatus);
    vi.mocked(backend.getVipStatus).mockResolvedValue({ active: false });
    vi.mocked(backend.getBackupStatus).mockResolvedValue({ databasePath: "", backupDirectory: "", backups: [] });
    vi.mocked(backend.getTrayIconStyle).mockResolvedValue(style);
    vi.mocked(backend.getWorkTimeSettings).mockResolvedValue({ gapMinutes: 45 });
    await renderView();
    expect(backend.getTrayIconStyle).toHaveBeenCalledOnce();
    expect(wrapper.get(`.style-${style.toLowerCase()}`).attributes("aria-checked")).toBe("true");
  });
});
