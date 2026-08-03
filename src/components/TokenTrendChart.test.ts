import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import TokenTrendChart from "./TokenTrendChart.vue";

const points = [
  { date: "2026-08-02", inputTokens: 800, cachedInputTokens: 300, outputTokens: 200, reasoningOutputTokens: 50, totalTokens: 1000 },
  { date: "2026-08-03", inputTokens: 1600, cachedInputTokens: 700, outputTokens: 400, reasoningOutputTokens: 100, totalTokens: 2000 },
];

describe("TokenTrendChart", () => {
  it("显示纵坐标并在悬停时展示 Token 构成", async () => {
    const wrapper = mount(TokenTrendChart, { props: { points } });
    expect(wrapper.findAll(".chart-y-label")).toHaveLength(5);
    await wrapper.findAll(".chart-hit")[1].trigger("mouseenter");
    expect(wrapper.find(".chart-tooltip").text()).toContain("2026-08-03");
    expect(wrapper.find(".chart-tooltip").text()).toContain("2,000 Token");
    expect(wrapper.find(".chart-tooltip").text()).toContain("缓存 700");
  });

  it("点击数据列会返回对应日期", async () => {
    const wrapper = mount(TokenTrendChart, { props: { points } });
    await wrapper.findAll(".chart-hit")[0].trigger("click");
    expect(wrapper.emitted("select")?.[0]).toEqual([points[0]]);
  });
});
