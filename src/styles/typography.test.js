import { describe, expect, it } from "vitest";
import postcss from "postcss";
import { readFileSync } from "node:fs";

// 本轮增强从该注释开始；已有字号选择器和字号换算不在审计范围内。
const typography = readFileSync("src/styles/typography.css", "utf8");
const layout = postcss.parse(typography.slice(typography.indexOf("/* 标题与说明")));
const scope = ':root:is([data-font-size="large"], [data-font-size="extra-large"])';
function declarations(selector) {
  const values = {};
  layout.walkRules(`${scope} ${selector}`, rule => {
    rule.walkDecls(decl => { values[decl.prop] = decl.value; });
  });
  return values;
}

describe("大字号排版隔离与容纳规则", () => {
  it("每一条新增布局规则均明确限定大/超大，不影响默认中字号和小字号", () => {
    let count = 0;
    layout.walkRules(rule => {
      // 检查每个逗号分隔选择器，防止只给第一项加上限制。
      for (const selector of rule.selectors) {
        expect(selector.startsWith(scope) || selector.startsWith(':root[data-font-size="extra-large"]')).toBe(true);
        expect(selector).not.toContain('[data-font-size="medium"]');
        expect(selector).not.toContain('[data-font-size="small"]');
        count++;
      }
    });
    expect(count).toBeGreaterThan(100);
  });
  it("不修改字号偏移、不整页缩放、不改变图标尺寸", () => {
    layout.walkDecls(decl => {
      expect(["font-size", "--app-font-offset", "zoom", "transform", "--control-icon-size"]).not.toContain(decl.prop);
    });
  });
  it("统计卡片说明回到文档流，标题和趋势卡片随内容增高", () => {
    expect(declarations(".metric-grid article > p").position).toBe("static");
    expect(declarations(".metric-grid article").height).toBe("auto");
    expect(declarations(".panel-head").height).toBe("auto");
    expect(declarations(".large-token-chart")).toMatchObject({ height: "auto", "min-height": "430px" });
  });
  it("抽屉与弹窗可滚动且受视口高度约束", () => {
    expect(declarations(".task-editor")["overflow-y"]).toBe("auto");
    expect(declarations(".test-config-dialog")["max-height"]).toBe("calc(100dvh - 40px)");
    expect(declarations(".quick-capture-body")).toMatchObject({ "min-height": "0", "overflow-y": "auto" });
    expect(declarations(".workbench-confirm-dialog > p")["overflow-y"]).toBe("auto");
  });
  it("发布配置贴满抽屉高度，仅表单滚动，底部操作不被挤出", () => {
    expect(declarations(".jenkins-config-dialog")).toMatchObject({ height: "100%", "max-height": "100dvh", display: "flex", "flex-direction": "column" });
    expect(declarations(".jenkins-config-dialog > div")).toMatchObject({ flex: "1", "min-height": "0", "overflow-y": "auto", "align-content": "start" });
    expect(declarations(".jenkins-config-dialog > footer")).toMatchObject({ "flex-shrink": "0", height: "auto", "min-height": "64px" });
  });
});
