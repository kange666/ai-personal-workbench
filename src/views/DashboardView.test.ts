import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import postcss from "postcss";
import DashboardView from "./DashboardView.vue";
import dashboardSource from "./DashboardView.vue?raw";
import * as backend from "../services/backend";

vi.mock("../services/backend");
let wrapper: VueWrapper;
const sampleTests = Array.from({ length: 5 }, (_, index) => ({
  id: `sample-${index}`, menuName: index === 0 ? "很长的测试菜单名称".repeat(8) : `示例测试 ${index + 1}`,
  project: "示例项目", status: index === 0 ? "failed" : "passed", startedAt: "2026-09-03T09:00:00Z",
})) as backend.TestRun[];

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(backend.isTauriRuntime).mockReturnValue(true);
  vi.mocked(backend.getTokenTrend).mockResolvedValue([]);
  vi.mocked(backend.getDailyActivity).mockResolvedValue([]);
  vi.mocked(backend.listReports).mockResolvedValue([]);
  vi.mocked(backend.listInboxItems).mockResolvedValue([]);
  vi.mocked(backend.getWorkSummary).mockResolvedValue({ totalMinutes: 0, byProject: [] } as unknown as backend.WorkSummary);
});
afterEach(() => { wrapper?.unmount(); });

async function render(tests: backend.TestRun[]) {
  vi.mocked(backend.listTestRuns).mockResolvedValue(tests);
  const router = createRouter({ history: createMemoryHistory(), routes: [
    { path: "/", component: DashboardView },
    { path: "/:pathMatch(.*)*", component: { template: "<div/>" } },
  ] });
  await router.push("/");
  await router.isReady();
  wrapper = mount(DashboardView, { global: { plugins: [router], stubs: { ActivityTrendChart: true, WorkTimeDrawer: true, BrandWordmark: true } } });
  await flushPromises();
  return router;
}

describe("首页最近测试卡片", () => {
  it("四条记录全部放入独立的可键盘滚动区域，不删减第四条", async () => {
    await render(sampleTests);
    const card = wrapper.get(".risk-panel");
    const list = card.get('.dashboard-test-list[role="region"]');
    expect(list.attributes("tabindex")).toBe("0");
    expect(list.attributes("aria-label")).toBe("最近测试记录");
    expect(list.findAll(".dashboard-risk")).toHaveLength(4);
    expect(list.text()).toContain("示例测试 4");
    expect(list.text()).not.toContain("示例测试 5");
    expect(card.findAll(":scope > .dashboard-risk")).toHaveLength(0);
    expect(list.get("b").attributes("title")).toBe(sampleTests[0].menuName);
    expect(list.get("small").attributes("title")).toContain("测试失败");
  });
  it("记录仍可跳转测试中心", async () => {
    const router = await render(sampleTests);
    await wrapper.get(".dashboard-test-list .dashboard-risk").trigger("click");
    await flushPromises();
    expect(router.currentRoute.value.path).toBe("/testing");
  });
  it("没有记录时保留空状态，不显示空滚动区域", async () => {
    await render([]);
    expect(wrapper.get(".risk-panel").text()).toContain("当前没有测试记录");
    expect(wrapper.find(".dashboard-test-list").exists()).toBe(false);
  });
  it("卡片和列表具备收缩与滚动约束，行高可随文字增长", () => {
    const styles = postcss.parse(dashboardSource.split("<style scoped>")[1].split("</style>")[0]);
    function declarations(selector: string) {
      const result: Record<string, string> = {};
      styles.walkRules(selector, (rule) => { rule.walkDecls((declaration) => { result[declaration.prop] = declaration.value; }); });
      return result;
    }
    expect(declarations(".risk-panel")).toMatchObject({ "min-height": "0", display: "flex", "flex-direction": "column", overflow: "hidden" });
    expect(declarations(".dashboard-test-list")).toMatchObject({ "min-height": "0", "overflow-y": "auto", "overflow-x": "hidden" });
    expect(declarations(".dashboard-test-list>.dashboard-risk")).toMatchObject({ height: "auto", "min-height": "48px" });
  });
});
