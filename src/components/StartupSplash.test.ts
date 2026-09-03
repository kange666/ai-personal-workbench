import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import StartupSplash from "./StartupSplash.vue";

describe("StartupSplash", () => {
  it("使用独立的全屏启动态展示品牌和首批数据进度", () => {
    const wrapper = mount(StartupSplash);
    expect(wrapper.get(".startup-splash").attributes("role")).toBe("status");
    expect(wrapper.text()).toContain("星枢工作台");
    expect(wrapper.text()).toContain("正在连接你的项目、记录与工作状态");
    expect(wrapper.find(".workbench-page-loader").exists()).toBe(false);
  });

  it("启动较慢时明确说明仍在后台整理", () => {
    const wrapper = mount(StartupSplash, { props:{ slow:true } });
    expect(wrapper.text()).toContain("本地数据较多");
  });
});
