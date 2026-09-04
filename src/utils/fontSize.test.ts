import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import postcss from "postcss";
import { fontSizePlugin } from "../../scripts/font-size-plugin";
import { applyFontSize, fontSizeOptions, loadFontSize, saveFontSize } from "./fontSize";

beforeEach(() => { localStorage.clear(); });
afterEach(() => { vi.restoreAllMocks(); applyFontSize("medium"); localStorage.clear(); });

describe("页面字号偏好", () => {
  it("默认中号，旧配置或不可读配置安全回退", () => {
    expect(loadFontSize()).toBe("medium");
    localStorage.setItem("workbench-font-size-v1", "unknown");
    expect(loadFontSize()).toBe("medium");
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw new Error("denied"); });
    expect(loadFontSize()).toBe("medium");
  });
  it.each(fontSizeOptions)("$label 立即应用偏移 $offset px，重新加载可恢复", ({ value, offset }) => {
    saveFontSize(value);
    expect(document.documentElement.style.getPropertyValue("--app-font-offset")).toBe(`${offset}px`);
    expect(document.documentElement.dataset.fontSize).toBe(value);
    expect(loadFontSize()).toBe(value);
    applyFontSize("medium");
    applyFontSize(loadFontSize());
    expect(document.documentElement.dataset.fontSize).toBe(value);
  });
  it("保存失败不改变当前字号", () => {
    applyFontSize("medium");
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("denied"); });
    expect(() => saveFontSize("large")).toThrow("denied");
    expect(document.documentElement.dataset.fontSize).toBe("medium");
  });
});

describe("全局字号编译规则", () => {
  const transform = async (css: string) => (await postcss([fontSizePlugin()]).process(css, { from: undefined })).css;
  it("普通、scoped 和响应式样式都使用偏移并限制最小 10px", async () => {
    const result = await transform("body{font-size:13px}.hint[data-v-demo]{font-size:8px!important}@media(max-width:1280px){h2{font-size:18px}}");
    expect(result).toContain("max(10px, calc(13px + var(--app-font-offset, 0px)))");
    expect(result).toContain("max(10px, calc(8px + var(--app-font-offset, 0px)))!important");
    expect(result).toContain("max(10px, calc(18px + var(--app-font-offset, 0px)))");
  });
  it("字体简写只修改字号，不修改行高或字体族", async () => {
    expect(await transform('pre{font:italic 700 12px/20px "Consolas",monospace}'))
      .toBe('pre{font:italic 700 max(10px, calc(12px + var(--app-font-offset, 0px)))/20px "Consolas",monospace}');
  });
  it("继承、图标、边距、宽高、字体名称和 CSS 变量保持原样", async () => {
    const css = '.icon{font-size:var(--control-icon-size);width:16px;height:16px;padding:2px}b{font:inherit;font-size:inherit}code{font:1em "12px"}';
    expect(await transform(css)).toBe(css);
  });
  it("重复编译不重复增大字号", async () => {
    const css = await transform("p{font-size:13px}pre{font:12px/1.5 monospace}");
    expect(await transform(css)).toBe(css);
  });
});
