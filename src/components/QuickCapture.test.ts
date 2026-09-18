import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { QuickCapture as CaptureRecord } from "../services/backend";

const backend = vi.hoisted(() => ({
  isTauriRuntime: vi.fn(() => true),
  listQuickCaptures: vi.fn(async ():Promise<CaptureRecord[]> => []),
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
    backend.saveQuickCapture.mockReset();
    backend.listQuickCaptures.mockResolvedValue([]);
  });

  const existing:CaptureRecord={id:"capture-url-1",kind:"url",content:"参考文档",sourceUrl:"https://example.com/old",status:"inbox",createdAt:"2026-09-01T10:00:00Z",updatedAt:"2026-09-01T10:00:00Z"};
  async function openExisting() {
    backend.listQuickCaptures.mockResolvedValue([{...existing}]);
    const wrapper=mount(QuickCapture,{props:{open:false}});
    await wrapper.setProps({open:true});
    await flushPromises();
    return wrapper;
  }

  it("编辑原记录与链接后原位更新，不增加记录数量",async()=>{
    const wrapper=await openExisting();
    await wrapper.get(".quick-capture-actions button").trigger("click");
    expect((wrapper.get("textarea").element as HTMLTextAreaElement).value).toBe(existing.content);
    expect((wrapper.get("input").element as HTMLInputElement).value).toBe(existing.sourceUrl);
    const updated={...existing,content:"更新后的文档",sourceUrl:"https://example.com/new",updatedAt:"2026-09-18T10:00:00Z"};
    backend.saveQuickCapture.mockResolvedValue(updated);
    await wrapper.get("textarea").setValue(updated.content);
    await wrapper.get("input").setValue(updated.sourceUrl);
    await wrapper.get(".quick-capture-editor footer .primary").trigger("click");
    await flushPromises();
    expect(backend.saveQuickCapture).toHaveBeenCalledWith({id:existing.id,kind:"url",content:updated.content,sourceUrl:updated.sourceUrl});
    expect(wrapper.findAll(".quick-capture-list article")).toHaveLength(1);
    expect(wrapper.get(".quick-capture-copy p").text()).toBe(updated.content);
    expect(wrapper.get(".quick-capture-copy a").attributes("href")).toBe(updated.sourceUrl);
    expect(wrapper.emitted("saved")?.[0]).toEqual([updated]);
    wrapper.unmount();
  });

  it("取消编辑保留原记录，并恢复未保存的新记录草稿",async()=>{
    const wrapper=await openExisting();
    await wrapper.get("textarea").setValue("尚未保存的新笔记");
    await wrapper.get(".quick-capture-actions button").trigger("click");
    await wrapper.get("textarea").setValue("准备取消的修改");
    await wrapper.get(".quick-capture-editor footer .secondary").trigger("click");
    expect((wrapper.get("textarea").element as HTMLTextAreaElement).value).toBe("尚未保存的新笔记");
    expect(wrapper.get(".quick-capture-editor nav .active").text()).toBe("笔记");
    expect(wrapper.get(".quick-capture-copy p").text()).toBe(existing.content);
    expect(backend.saveQuickCapture).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it("保存失败保留编辑内容，重复点击编辑中不会丢失修改",async()=>{
    const wrapper=await openExisting();
    await wrapper.get(".quick-capture-actions button").trigger("click");
    await wrapper.get("textarea").setValue("待重试的修改");
    await wrapper.get(".quick-capture-actions button").trigger("click");
    expect((wrapper.get("textarea").element as HTMLTextAreaElement).value).toBe("待重试的修改");
    backend.saveQuickCapture.mockRejectedValue(new Error("保存失败"));
    await wrapper.get("textarea").trigger("keydown",{key:"Enter",ctrlKey:true});
    await flushPromises();
    expect(wrapper.get(".form-error").text()).toContain("保存失败");
    expect(wrapper.get(".quick-capture-editor footer .primary").text()).toBe("保存修改");
    expect((wrapper.get("textarea").element as HTMLTextAreaElement).value).toBe("待重试的修改");
    expect(wrapper.get(".quick-capture-copy p").text()).toBe(existing.content);
    wrapper.unmount();
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
