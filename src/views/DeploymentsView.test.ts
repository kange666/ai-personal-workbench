import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import DeploymentsView from "./DeploymentsView.vue";

const mocks=vi.hoisted(()=>({
  jobs:[
    {name:"普通项目",fullName:"folder/普通项目",url:"https://jenkins/job/folder/job/normal/",className:"job",favorite:false},
    {name:"常用项目",fullName:"folder/常用项目",url:"https://jenkins/job/folder/job/favorite/",className:"job",favorite:true},
  ],
  favorite:vi.fn(async()=>undefined),
  branches:vi.fn(async(jobFullName:string)=>({jobFullName,parameterName:"BRANCH_NAME",branches:["main","develop"]})),
  publish:vi.fn(async(jobFullName:string,branch:string)=>({id:"run-1",jobName:"普通项目",jobFullName,jobUrl:"https://jenkins/job/normal/",branchParameter:"BRANCH_NAME",branch,queueId:12,queueUrl:"https://jenkins/queue/item/12/",buildUrl:"",status:"queued",syncState:"synced",queueReason:"等待节点",currentStage:"",stages:[],startedAt:"2026-09-01T08:00:00Z",updatedAt:"2026-09-01T08:00:00Z",result:"",errorMessage:""})),
}));

vi.mock("@tauri-apps/api/event",()=>({listen:vi.fn(async()=>vi.fn())}));
vi.mock("../services/backend",()=>({
  isTauriRuntime:()=>true,
  getJenkinsConnectionStatus:async()=>({configured:true,baseUrl:"https://jenkins",username:"tester",version:"2.500",lastVerifiedAt:"2026-09-01T08:00:00Z"}),
  listJenkinsJobs:async()=>mocks.jobs.map(item=>({...item})),
  listJenkinsJobBranches:mocks.branches,
  setJenkinsJobFavorite:mocks.favorite,
  triggerJenkinsPublish:mocks.publish,
  listJenkinsPublishRecords:async()=>[],
  openJenkinsUrl:vi.fn(),
  saveJenkinsConnection:vi.fn(),
  testJenkinsConnection:vi.fn(),
}));

async function mountView() {
  const router=createRouter({history:createMemoryHistory(),routes:[{path:"/deployments",component:DeploymentsView}]});
  await router.push("/deployments");await router.isReady();
  const wrapper=mount(DeploymentsView,{global:{plugins:[router]}});
  await flushPromises();
  return wrapper;
}

describe("DeploymentsView",()=>{
  beforeEach(()=>{
    mocks.favorite.mockClear();
    mocks.branches.mockClear();
    mocks.publish.mockClear();
    vi.stubGlobal("confirm",vi.fn(()=>true));
  });

  it("收藏项目固定显示在项目列表顶部并可取消收藏",async()=>{
    const wrapper=await mountView();
    await wrapper.get(".project-select-trigger").trigger("click");
    const sections=wrapper.findAll(".project-picker-list section");
    expect(sections[0].text()).toContain("已收藏");
    expect(sections[0].text()).toContain("常用项目");
    expect(sections[1].text()).toContain("普通项目");
    await sections[0].get("i.active").trigger("click");
    await flushPromises();
    expect(mocks.favorite).toHaveBeenCalledWith("folder/常用项目",false);
    expect(wrapper.find(".project-picker-list section").text()).not.toContain("已收藏");
    wrapper.unmount();
  });

  it("选择项目后读取 Jenkins 分支并只用所选分支发布",async()=>{
    const wrapper=await mountView();
    await wrapper.get(".project-select-trigger").trigger("click");
    const normalButton=wrapper.findAll(".project-picker-list button").find(button=>button.text().includes("普通项目"))!;
    await normalButton.trigger("click");await flushPromises();
    expect(mocks.branches).toHaveBeenCalledWith("folder/普通项目");
    expect(wrapper.text()).toContain("参数：BRANCH_NAME");
    await wrapper.get("select").setValue("develop");
    await wrapper.get(".publish-button").trigger("click");await flushPromises();
    expect(window.confirm).toHaveBeenCalled();
    expect(mocks.publish).toHaveBeenCalledWith("folder/普通项目","develop");
    expect(wrapper.text()).toContain("已进入 Jenkins 队列");
    wrapper.unmount();
  });
});
