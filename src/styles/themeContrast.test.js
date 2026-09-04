import { readFileSync } from "node:fs";
import postcss from "postcss";
import { describe, expect, it } from "vitest";

const css = postcss.parse(readFileSync("src/styles/main.css", "utf8"));
const activityChart = readFileSync("src/components/ActivityTrendChart.vue", "utf8");
const activityAlpha = Number(/\.activity-bar\{fill:color-mix\(in srgb,var\(--primary\) (\d+)%/.exec(activityChart)[1]) / 100;
const themes = {};
for (const selector of [":root", ':root[data-theme="warm"]']) {
  const values = {};
  css.walkRules(selector, rule => { rule.walkDecls(decl => { values[decl.prop] = decl.value; }); });
  themes[selector] = values;
}
function rgb(hex) {
  return [1, 3, 5].map(index => parseInt(hex.slice(index, index + 2), 16));
}
function resolveColor(values, name) {
  const value = values[name];
  const variable = /^var\((--[\w-]+)\)$/.exec(value);
  return variable ? resolveColor(values, variable[1]) : rgb(value);
}
function luminance(color) {
  const linear = color.map(value => {
    const channel = value / 255;
    return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
  });
  return linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722;
}
function contrast(a, b) {
  const values = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (values[0] + 0.05) / (values[1] + 0.05);
}

describe("主题信息文字的实际色值对比", () => {
  for (const [theme, values] of Object.entries(themes)) {
    it(`${theme}：半透明活动柱形与实际卡片底色达到 3:1`, () => {
      const foreground = rgb(values["--primary"]);
      const background = rgb(values["--surface"]);
      const composited = foreground.map((value, index) => value * activityAlpha + background[index] * (1 - activityAlpha));
      expect(contrast(composited, background)).toBeGreaterThanOrEqual(3);
    });
    it(`${theme}：实心按钮及悬停状态的白字达到 4.5:1`, () => {
      for (const token of ["--primary-fill", "--primary-fill-hover"]) {
        expect(contrast([255, 255, 255], resolveColor(values, token)), token).toBeGreaterThanOrEqual(4.5);
      }
    });
    it(`${theme}：正文、辅助文字和状态色在常见卡片底色上达到 4.5:1`, () => {
      for (const foreground of ["--text", "--muted", "--primary", "--success", "--warning", "--danger"]) {
        for (const background of ["--bg", "--surface", "--surface-2", "--surface-3", "--primary-soft"]) {
          expect(contrast(rgb(values[foreground]), rgb(values[background])), `${foreground}/${background}`).toBeGreaterThanOrEqual(4.5);
        }
      }
    });
    it(`${theme}：接口方法标签在混合背景上仍可阅读`, () => {
      for (const [name, alpha] of [["--success", 0.13], ["--primary", 0.08], ["--warning", 0.14], ["--danger", 0.13]]) {
        const foreground = rgb(values[name]);
        const surface = rgb(values["--surface"]);
        const mixed = foreground.map((channel, index) => channel * alpha + surface[index] * (1 - alpha));
        expect(contrast(foreground, mixed), name).toBeGreaterThanOrEqual(4.5);
      }
    });
  }
});
