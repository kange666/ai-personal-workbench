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
  records:[] as any[],
}));

vi.mock("@tauri-apps/api/event",()=>({listen:vi.fn(async()=>vi.fn())}));
vi.mock("../services/backend",()=>({
  isTauriRuntime:()=>true,
  getJenkinsConnectionStatus:async()=>({configured:true,baseUrl:"https://jenkins",username:"tester",version:"2.500",lastVerifiedAt:"2026-09-01T08:00:00Z"}),
  listJenkinsJobs:async()=>mocks.jobs.map(item=>({...item})),
  listJenkinsJobBranches:mocks.branches,
  setJenkinsJobFavorite:mocks.favorite,
  triggerJenkinsPublish:mocks.publish,
  listJenkinsPublishRecords:async()=>mocks.records,
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

function publishRecord(id:string,status:"queued"|"running"|"success"|"failed"|"aborted",overrides:Record<string,unknown>={}) {
  return {id,jobName:"普通项目",jobFullName:"folder/普通项目",jobUrl:"https://jenkins/job/normal/",branchParameter:"BRANCH_NAME",branch:"develop",queueId:12,queueUrl:"https://jenkins/queue/item/12/",buildNumber:18,buildUrl:"https://jenkins/job/normal/18/",status,syncState:"synced",queueReason:"",currentStage:"部署",stages:[],startedAt:new Date().toISOString(),finishedAt:status==="queued"||status==="running"?undefined:new Date().toISOString(),updatedAt:new Date().toISOString(),result:status.toUpperCase(),errorMessage:"",...overrides};
}

describe("DeploymentsView",()=>{
  beforeEach(()=>{
    mocks.favorite.mockClear();
    mocks.branches.mockClear();
    mocks.publish.mockClear();
    mocks.records=[];
    window.localStorage.clear();
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
    expect(JSON.parse(window.localStorage.getItem("workbench-jenkins-last-project") || "{}")).toEqual({baseUrl:"https://jenkins",jobFullName:"folder/普通项目"});
    expect(wrapper.text()).toContain("参数：BRANCH_NAME");
    await wrapper.get("select").setValue("develop");
    await wrapper.get(".publish-button").trigger("click");await flushPromises();
    expect(window.confirm).toHaveBeenCalled();
    expect(mocks.publish).toHaveBeenCalledWith("folder/普通项目","develop");
    expect(wrapper.text()).toContain("已进入 Jenkins 队列");
    wrapper.unmount();
  });

  it("恢复上次选择的项目，但要求重新确认分支",async()=>{
    window.localStorage.setItem("workbench-jenkins-last-project",JSON.stringify({baseUrl:"https://jenkins",jobFullName:"folder/普通项目"}));
    mocks.branches.mockResolvedValueOnce({jobFullName:"folder/普通项目",parameterName:"BRANCH_NAME",branches:["main"]});
    const wrapper=await mountView();
    expect(wrapper.get(".project-select-trigger").text()).toContain("folder/普通项目");
    expect((wrapper.get("select").element as HTMLSelectElement).value).toBe("");
    expect(wrapper.get(".publish-button").attributes("disabled")).toBeDefined();
    wrapper.unmount();
  });

  it("同项目同分支发布中时禁用重复发布",async()=>{
    mocks.records=[publishRecord("active-1","running")];
    const wrapper=await mountView();
    await wrapper.get(".project-select-trigger").trigger("click");
    const normalButton=wrapper.findAll(".project-picker-list button").find(button=>button.text().includes("普通项目"))!;
    await normalButton.trigger("click");await flushPromises();
    await wrapper.get("select").setValue("develop");
    expect(wrapper.get(".publish-button").text()).toBe("正在发布");
    expect(wrapper.get(".publish-button").attributes("disabled")).toBeDefined();
    expect(wrapper.text()).toContain("该项目和分支正在发布");
    wrapper.unmount();
  });

  it("发布记录可按结果、项目名称和时间范围筛选",async()=>{
    const oldDate=new Date(Date.now()-10*24*60*60*1000).toISOString();
    mocks.records=[
      publishRecord("success-today","success"),
      publishRecord("failed-today","failed",{jobName:"后台服务",jobFullName:"folder/后台服务"}),
      publishRecord("success-old","success",{startedAt:oldDate,finishedAt:oldDate,updatedAt:oldDate}),
    ];
    const wrapper=await mountView();
    expect(wrapper.findAll(".deployment-history>article")).toHaveLength(3);
    await wrapper.get('[aria-label="筛选发布结果"]').setValue("failed");
    expect(wrapper.findAll(".deployment-history>article")).toHaveLength(1);
    expect(wrapper.get(".deployment-history>article").text()).toContain("后台服务");
    await wrapper.get('[aria-label="筛选发布结果"]').setValue("all");
    await wrapper.get('[aria-label="搜索发布项目"]').setValue("普通项目");
    expect(wrapper.findAll(".deployment-history>article")).toHaveLength(2);
    await wrapper.get('[aria-label="筛选发布时间"]').setValue("today");
    expect(wrapper.findAll(".deployment-history>article")).toHaveLength(1);
    wrapper.unmount();
  });
});
