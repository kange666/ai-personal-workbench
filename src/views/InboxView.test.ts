import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { describe, expect, it, vi } from "vitest";

const backend = vi.hoisted(() => ({
  listInboxItems: vi.fn(async () => [
    {
      id: "quick_capture:capture-1",
      sourceType: "quick_capture",
      sourceId: "capture-1",
      project: "未归类项目",
      title: "整理发布说明",
      summary: "整理发布说明",
      detail: "来自快速记录",
      route: "",
      priority: "normal",
      workflowStatus: "needs_decision",
      sourceStatus: "待处理",
      createdAt: "2026-09-02T10:00:00Z",
      updatedAt: "2026-09-02T10:00:00Z",
    },
    {
      id: "video:video-1",
      sourceType: "video",
      sourceId: "video-1",
      project: "视频创作",
      title: "视频需要处理",
      summary: "视频生成失败",
      detail: "查看视频中心",
      route: "/videos?job=video-1",
      priority: "high",
      workflowStatus: "needs_decision",
      sourceStatus: "failed",
      createdAt: "2026-09-02T10:00:00Z",
      updatedAt: "2026-09-02T10:00:00Z",
    },
  ]),
  updateInboxStatus: vi.fn(),
  createTaskFromInbox: vi.fn(),
}));

vi.mock("../services/backend", () => ({
  isTauriRuntime: () => true,
  ...backend,
}));

import InboxView from "./InboxView.vue";

describe("InboxView", () => {
  it("展示快速任务并防御性过滤视频内容", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/inbox", component: InboxView },
        { path: "/calendar", component: { template: "<div />" } },
      ],
    });
    await router.push("/inbox");
    await router.isReady();
    const wrapper = mount(InboxView, { global: { plugins: [router] } });
    await flushPromises();

    expect(wrapper.text()).toContain("整理发布说明");
    expect(wrapper.text()).toContain("快速任务");
    expect(wrapper.text()).not.toContain("视频需要处理");
    expect(wrapper.text()).not.toContain("视频生成失败");
  });
});
