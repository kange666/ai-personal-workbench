import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as backend from "../services/backend";
import SettingsView from "./SettingsView.vue";

vi.mock("../services/backend");
vi.mock("../services/updateStatus", () => ({ refreshUpdateStatus: vi.fn(async () => ({})) }));
vi.mock("@tauri-apps/plugin-autostart", () => ({ isEnabled: vi.fn(async () => false) }));
vi.mock("vue-router", () => ({ useRoute: () => ({ query: {} }) }));

let wrapper: VueWrapper;
async function renderView() {
  wrapper = mount(SettingsView, {
    global: { stubs: { ThemeSwitch: true, NavIcon: true, RouterLink: true } },
  });
  await flushPromises();
  return wrapper;
}

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(backend.isTauriRuntime).mockReturnValue(false);
  vi.mocked(backend.setTrayIconStyle).mockImplementation(async (style) => style);
});
afterEach(() => { wrapper?.unmount(); });

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
