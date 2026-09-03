import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import BrandWordmark from "./BrandWordmark.vue";
import { APP_BRAND } from "../utils/brand";
import tauriConfig from "../../src-tauri/tauri.conf.json";

describe("BrandWordmark", () => {
  it.each(["compact", "hero", "splash"] as const)("%s 版式沿用同一个中英文品牌", variant => {
    const wrapper = mount(BrandWordmark, { props: { variant } });
    expect(wrapper.classes()).toContain(`brand-wordmark--${variant}`);
    expect(wrapper.get(".brand-wordmark__chinese").text()).toBe(APP_BRAND.chinese);
    expect(wrapper.get(".brand-wordmark__english").text()).toBe(APP_BRAND.english);
    expect(wrapper.get(".brand-wordmark__english").attributes("lang")).toBe("en");
    expect(wrapper.text()).not.toContain("工作台");
  });

  it("默认使用适配窄侧栏的紧凑版式", () => {
    expect(mount(BrandWordmark).classes()).toContain("brand-wordmark--compact");
  });

  it("更新窗口显示名时保留安装身份和数据目录标识", () => {
    expect(tauriConfig.app.windows[0].title).toBe(APP_BRAND.name);
    expect(tauriConfig.identifier).toBe("com.local.ai-personal-workbench");
    expect(tauriConfig.productName).toBe("星枢工作台");
  });
});
