import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

const backend = vi.hoisted(() => ({
  isTauriRuntime: vi.fn(() => true),
  listQuickCaptures: vi.fn(async () => []),
  saveQuickCapture: vi.fn(),
  archiveQuickCapture: vi.fn(),
  deleteQuickCapture: vi.fn(),
}));

vi.mock("../services/backend", () => backend);
vi.mock("../utils/confirm", () => ({ confirmAction: vi.fn(async () => true) }));

import QuickCapture from "./QuickCapture.vue";

describe("QuickCapture", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    backend.listQuickCaptures.mockResolvedValue([]);
  });

  it("任务保存后进入待处理收件箱且不留在待整理列表", async () => {
    backend.saveQuickCapture.mockResolvedValue({
      id: "capture-task-1",
      kind: "task",
      content: "整理本周发布说明",
      sourceUrl: "",
      status: "routed",
      createdAt: "2026-09-02T10:00:00Z",
      updatedAt: "2026-09-02T10:00:00Z",
    });
    const wrapper = mount(QuickCapture, { props: { open: false } });
    await wrapper.setProps({ open: true });
    await flushPromises();

    await wrapper.findAll(".quick-capture-editor nav button").find(button => button.text() === "任务")!.trigger("click");
    await wrapper.get(".quick-capture-editor textarea").setValue("整理本周发布说明");
    await wrapper.findAll("button").find(button => button.text() === "加入待处理")!.trigger("click");
    await flushPromises();

    expect(backend.saveQuickCapture).toHaveBeenCalledWith({
      kind: "task",
      content: "整理本周发布说明",
      sourceUrl: "",
    });
    expect(wrapper.get(".quick-capture-message").text()).toBe("任务已加入待处理收件箱。");
    expect(wrapper.findAll(".quick-capture-list article")).toHaveLength(0);
  });
});
