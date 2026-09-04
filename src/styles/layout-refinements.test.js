import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import postcss from "postcss";

const css = postcss.parse(readFileSync("src/styles/layout-refinements.css", "utf8"));
function declarations(selector) {
  const values = {};
  css.walkRules(rule => {
    if (rule.selectors.includes(selector)) rule.walkDecls(decl => { values[decl.prop] = decl.value; });
  });
  return values;
}

describe("默认字号布局回归", () => {
  it("不改变原字号、缩放、主题颜色和图标尺寸", () => {
    css.walkDecls(decl => {
      expect(["font-size", "--app-font-offset", "zoom", "transform", "color", "background", "--control-icon-size"]).not.toContain(decl.prop);
    });
  });
  it("只有标题的页面不套用操作组布局", () => {
    expect(declarations(".page-header > div:only-child").display).toBe("block");
  });
  it("发布抽屉填满高度，内容滚动且首尾不收缩", () => {
    expect(declarations(":root .jenkins-config-dialog")).toMatchObject({ height: "100%", display: "flex", "flex-direction": "column" });
    expect(declarations(":root .jenkins-config-dialog > div")).toMatchObject({ flex: "1", "min-height": "0", "overflow-y": "auto" });
    expect(declarations(":root .jenkins-config-dialog > footer")["flex-shrink"]).toBe("0");
  });
  it("长测试配置只滚动正文，不压缩表单或底部操作", () => {
    expect(declarations(":root .test-config-dialog")).toMatchObject({ display: "flex", "overflow": "hidden" });
    expect(declarations(":root .test-config-dialog > .test-config-body")).toMatchObject({ "min-height": "0", "overflow-y": "auto" });
    expect(declarations(":root .test-config-body > *")["flex-shrink"]).toBe("0");
    expect(declarations(":root .test-config-dialog > footer")["flex-shrink"]).toBe("0");
  });
  it("工时操作放在底部，搜索结果不超过视口", () => {
    expect(declarations(".worktime-editor > footer")["margin-top"]).toBe("auto");
    expect(declarations(".workspace-search")["max-height"]).toBe("min(650px, calc(100dvh - 114px))");
    expect(declarations(".workspace-search-results")["min-height"]).toBe("0");
  });
});
