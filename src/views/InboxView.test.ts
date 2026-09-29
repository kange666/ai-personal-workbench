import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createPinia } from "pinia";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkTask } from "../types/workbench";

const backend = vi.hoisted(() => ({
  listInboxItems: vi.fn(async () => [
    {
      id: "quick_capture:capture-1",
      sourceType: "quick_capture",
      sourceId: "capture-1",
      project: "未归类项目",
      title: "整理发布说明",
      summary: "\u001b[31m整理发布说明\u001b[39m",
      detail: "来自快速记录",
      route: "",
      priority: "normal",
      workflowStatus: "needs_decision",
      sourceStatus: "待处理",
      createdAt: "2026-09-02T10:00:00Z",
      updatedAt: "2026-09-02T10:00:00Z",
      linkedTaskId: null as string | null,
      linkedTaskStatus: null as string | null,
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
      linkedTaskId: null as string | null,
      linkedTaskStatus: null as string | null,
    },
  ]),
  updateInboxStatus: vi.fn(),
  createTaskFromInbox: vi.fn(),
  listTasks: vi.fn(), saveTask: vi.fn(), deleteTask: vi.fn(), syncTaskSuggestions: vi.fn(),
}));

vi.mock("../services/backend", () => ({
  isTauriRuntime: () => true,
  ...backend,
}));

import InboxView from "./InboxView.vue";
import TasksView from "./TasksView.vue";
import { useWorkbenchStore } from "../stores/workbench";

const task=(id:string,status:WorkTask["status"]="todo"):WorkTask=>({id,title:"整理发布说明",project:"匿名项目",scope:"day",status,priority:"P1",plannedDate:new Date().toLocaleDateString("sv-SE"),progress:status==="done"?100:0,note:"来自收件箱的匿名测试任务",source:"inbox",sourceId:"quick_capture:capture-1",createdAt:"2026-09-29T08:00:00Z",updatedAt:"2026-09-29T08:00:00Z"});
const wrappers:VueWrapper[]=[];

describe("InboxView", () => {
  beforeEach(() => {
    vi.clearAllMocks(); localStorage.clear();
    backend.listTasks.mockReset().mockResolvedValue([task("linked-1"),task("linked-2","doing"),task("linked-3","doing")]);
  });
  afterEach(()=>{wrappers.splice(0).forEach(wrapper=>wrapper.unmount());});

  async function renderInbox(initialTasks:WorkTask[]=[]) {
    const router = createRouter({ history:createMemoryHistory(), routes:[
      {path:"/inbox",component:InboxView}, {path:"/calendar",component:TasksView},
    ] });
    await router.push("/inbox");
    await router.isReady();
    const pinia=createPinia();
    const store=useWorkbenchStore(pinia);store.tasks=initialTasks;
    const wrapper=mount({template:"<RouterView />"},{global:{plugins:[pinia,router]}});
    wrappers.push(wrapper);
    await flushPromises();
    return {wrapper,router,store};
  }

  it("只有安排任务入口，创建后跳转到同一关联任务", async () => {
    backend.createTaskFromInbox.mockResolvedValueOnce("linked-1");
    const {wrapper,router,store}=await renderInbox();
    expect(wrapper.text()).not.toContain("开始处理");
    const button=wrapper.findAll("button").find(button=>button.text()==="安排任务")!;
    await button.trigger("click");
    await flushPromises();
    expect(backend.createTaskFromInbox).toHaveBeenCalledExactlyOnceWith("quick_capture:capture-1");
    expect(router.currentRoute.value.query.task).toBe("linked-1");
    expect(store.tasks.find(item=>item.id==="linked-1")?.title).toBe("整理发布说明");
    expect(wrapper.findComponent(TasksView).exists()).toBe(true);
    expect(wrapper.get(".task-editor h2").text()).toBe("任务详情");
    expect((wrapper.get('.task-editor input[placeholder="输入任务名称"]').element as HTMLInputElement).value).toBe("整理发布说明");
  });

  it("已关联任务直接查看，不再调用转任务命令", async () => {
    const [item]=await backend.listInboxItems();
    backend.listInboxItems.mockResolvedValueOnce([{...item,workflowStatus:"in_progress",linkedTaskId:"linked-2",linkedTaskStatus:"doing"}]);
    const {wrapper,router}=await renderInbox();
    expect(wrapper.text()).toContain("任务·进行中");
    expect(wrapper.text()).not.toContain("安排任务");
    await wrapper.findAll("button").find(button=>button.text()==="查看任务")!.trigger("click");
    await flushPromises();
    expect(backend.createTaskFromInbox).not.toHaveBeenCalled();
    expect(router.currentRoute.value.query.task).toBe("linked-2");
  });

  it("复用已有建议任务时读取实际状态，不强制改成待办", async () => {
    backend.createTaskFromInbox.mockResolvedValueOnce("suggested-1");
    backend.listTasks.mockResolvedValueOnce([task("suggested-1","draft")]);
    const {wrapper,store}=await renderInbox();
    await wrapper.findAll("button").find(button=>button.text()==="安排任务")!.trigger("click");await flushPromises();
    expect(store.tasks.find(item=>item.id==="suggested-1")?.status).toBe("draft");
    expect(wrapper.get(".task-editor h2").text()).toBe("任务详情");
    expect(backend.saveTask).not.toHaveBeenCalled();
  });

  it("写入前已有慢查询时，等旧读取结束后再查询并打开新任务", async () => {
    let finishOld!: (value:WorkTask[])=>void;
    backend.listTasks.mockReturnValueOnce(new Promise<WorkTask[]>(resolve=>{finishOld=resolve;}));
    backend.createTaskFromInbox.mockResolvedValueOnce("linked-1");
    const {wrapper,router,store}=await renderInbox();
    const oldRead=store.hydrate();
    await wrapper.findAll("button").find(button=>button.text()==="安排任务")!.trigger("click");await flushPromises();
    expect(backend.listTasks).toHaveBeenCalledTimes(1);
    expect(router.currentRoute.value.path).toBe("/inbox");
    finishOld([]);await oldRead;await flushPromises();
    expect(backend.listTasks).toHaveBeenCalledTimes(2);
    expect(router.currentRoute.value.query.task).toBe("linked-1");
    expect(wrapper.get(".task-editor h2").text()).toBe("任务详情");
    expect(backend.createTaskFromInbox).toHaveBeenCalledTimes(1);
  });

  it("完成关联事项后可以重新处理，失败时保留原状态", async () => {
    const [item]=await backend.listInboxItems();
    backend.listInboxItems.mockResolvedValueOnce([{...item,workflowStatus:"done",linkedTaskId:"linked-3",linkedTaskStatus:"done"}]);
    const {wrapper,store}=await renderInbox([task("linked-3","done")]);
    await wrapper.findAll(".inbox-tabs button").find(button=>button.text().startsWith("已完成"))!.trigger("click");
    backend.updateInboxStatus.mockRejectedValueOnce(new Error("保存失败"));
    await wrapper.findAll("button").find(button=>button.text()==="重新处理")!.trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("保存失败");
    expect(wrapper.text()).toContain("任务·已完成");
    await wrapper.findAll("button").find(button=>button.text()==="重新处理")!.trigger("click");
    await flushPromises();
    expect(backend.updateInboxStatus).toHaveBeenLastCalledWith(item.id,"in_progress");
    await wrapper.findAll(".inbox-tabs button").find(button=>button.text().startsWith("处理中"))!.trigger("click");
    expect(wrapper.text()).toContain("任务·进行中");
    expect(store.tasks.find(item=>item.id==="linked-3")?.status).toBe("doing");
    await wrapper.findAll("button").find(button=>button.text()==="查看任务")!.trigger("click");
    await flushPromises();
    expect(wrapper.get(".task-editor h2").text()).toBe("任务详情");
    expect(store.tasks.find(item=>item.id==="linked-3")?.status).toBe("doing");
  });

  it("安排成功但同步失败保留关联，查看重试可打开且不重复安排", async () => {
    backend.createTaskFromInbox.mockResolvedValueOnce("linked-1");
    backend.listTasks.mockRejectedValueOnce(new Error("读取暂不可用"));
    const {wrapper,router,store}=await renderInbox();
    await wrapper.findAll("button").find(button=>button.text()==="安排任务")!.trigger("click");await flushPromises();
    expect(router.currentRoute.value.path).toBe("/inbox");
    expect(wrapper.get(".inbox-message").text()).toContain("任务已安排成功，但任务列表同步失败");
    expect(wrapper.get(".inbox-message").text()).not.toContain("保存失败");
    expect(wrapper.text()).not.toContain("安排任务");
    expect(store.tasks).toHaveLength(0);
    expect(wrapper.text()).toContain("任务·待同步");
    await wrapper.findAll("button").find(button=>button.text()==="查看任务")!.trigger("click");await flushPromises();
    expect(backend.createTaskFromInbox).toHaveBeenCalledTimes(1);
    expect(router.currentRoute.value.query.task).toBe("linked-1");
    expect(wrapper.get(".task-editor h2").text()).toBe("任务详情");
  });

  it("状态已保存但列表同步失败明确提示，查看重试同步已完成状态", async () => {
    const [item]=await backend.listInboxItems();
    backend.listInboxItems.mockResolvedValueOnce([{...item,workflowStatus:"in_progress",linkedTaskId:"linked-2",linkedTaskStatus:"doing"}]);
    backend.listTasks.mockRejectedValueOnce(new Error("读取暂不可用")).mockResolvedValueOnce([task("linked-2","done")]);
    const {wrapper,store}=await renderInbox([task("linked-2","doing")]);
    await wrapper.findAll("button").find(button=>button.text()==="标记完成")!.trigger("click");await flushPromises();
    expect(wrapper.get(".inbox-message").text()).toContain("事项状态已保存，但任务列表同步失败");
    expect(wrapper.get(".inbox-message").text()).not.toContain("保存失败");
    await wrapper.findAll(".inbox-tabs button").find(button=>button.text().startsWith("已完成"))!.trigger("click");
    expect(wrapper.text()).toContain("任务·已完成");
    await wrapper.findAll("button").find(button=>button.text()==="查看任务")!.trigger("click");await flushPromises();
    expect(store.tasks.find(item=>item.id==="linked-2")?.status).toBe("done");
    expect(wrapper.get(".task-editor h2").text()).toBe("任务详情");
    expect(backend.updateInboxStatus).toHaveBeenCalledTimes(1);
  });

  it("展示快速任务并防御性过滤视频内容", async () => {
    const {wrapper}=await renderInbox();

    expect(wrapper.text()).toContain("整理发布说明");
    expect(wrapper.text()).toContain("快速任务");
    expect(wrapper.get(".inbox-main p").text()).toBe("整理发布说明");
    expect(wrapper.text()).not.toContain("[31m");
    expect(wrapper.text()).not.toContain("视频需要处理");
    expect(wrapper.text()).not.toContain("视频生成失败");
  });
});
