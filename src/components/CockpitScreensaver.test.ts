import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import CockpitScreensaver from "./CockpitScreensaver.vue";

const mocks=vi.hoisted(()=>({
  getDailyActivity:vi.fn(async(start:string,end:string)=>{
    const result=[];const cursor=new Date(`${start}T12:00:00`);const last=new Date(`${end}T12:00:00`);
    while(cursor<=last){const date=cursor.toLocaleDateString("sv-SE");result.push({date,conversationCount:2,archivedConversationCount:0,messageCount:4,userMessages:2,assistantMessages:2,inputTokens:800,cachedInputTokens:200,outputTokens:100,reasoningOutputTokens:50,totalTokens:1150,gitCommits:1,contentIdeaCount:0,dailyReportId:`report-${date}`,workMinutes:60,estimatedWorkMinutes:60,manualWorkMinutes:0,testRuns:1,testsPassed:1,knowledgeCount:0,taskActivityCount:1,quickCaptureCount:0,completedVideoCount:0});cursor.setDate(cursor.getDate()+1);}
    return result;
  }),
}));

vi.mock("../services/backend",()=>({
  isTauriRuntime:()=>true,
  getDailyActivity:mocks.getDailyActivity,
  getWorkSummary:async()=>({startDate:"2026-08-31",endDate:"2026-09-01",totalMinutes:630,estimatedMinutes:630,manualMinutes:0,hasManualCorrections:false,byProject:[{name:"个人工作台",minutes:360},{name:"客户端",minutes:270}],byType:[],daily:[]}),
  listReports:async()=>Array.from({length:18},(_,index)=>({id:`report-${index}`,reportType:"daily",periodStart:"2026-09-01",periodEnd:"2026-09-01",title:`日报 ${index+1}`,contentMarkdown:"",status:"draft",createdAt:"2026-09-01T08:00:00",updatedAt:"2026-09-01T08:00:00"})),
  listWorkSessions:async()=>[{id:"work-1",date:"2026-09-01",startTime:"09:00",endTime:"10:00",durationMinutes:60,project:"个人工作台",workType:"Codex 开发",source:"estimated",note:"",createdAt:"2026-09-01T09:00:00",updatedAt:"2026-09-01T10:00:00"}],
}));

const notifications=Array.from({length:4},(_,index)=>({id:`notice-${index}`,kind:"codex_complete" as const,title:`消息 ${index+1}`,body:`消息内容 ${index+1}`,output:"",route:`/inbox?item=${index}`,isRead:false,createdAt:`2026-09-01T14:3${index}:00`,reviewStatus:"pending" as const,reviewNote:""}));
const runningProject={projectPath:"C:/workbench",projectName:"个人工作台",command:"npm run dev",processId:1,status:"running" as const,startedAt:"2026-09-01T08:00:00",localUrl:"http://localhost:1420",logPath:"",logExcerpt:"",errorMessage:""};
const runningTest={id:"test-1",menuId:"api",project:"个人工作台",projectPath:"C:/workbench",menuName:"接口回归",mode:"mock" as const,status:"running" as const,startedAt:"2026-09-01T14:00:00",reportMarkdown:"",outputExcerpt:"",errorMessage:"",selectedScenarios:["a","b"],scenarioResults:[],artifacts:[],totalCount:2,passedCount:1,failedCount:0,skippedCount:0,durationMs:0,environmentSummary:"",cleanupStatus:"unknown" as const};
const runningTapd={id:"tapd-1",itemKey:"workspace:415",itemId:"415",workspaceId:"workspace",repositoryPath:"C:/workbench",status:"running" as const,output:"",errorMessage:"",baselineHead:"",baselineWorktree:"",resultHead:"",changedFiles:[],testSummary:"",reviewStatus:"pending" as const,reviewNote:"",triggerSource:"auto" as const,sourceModifiedAt:"",triggerReason:"",executionMode:"automatic" as const,executionBlockReason:"",testRequired:true,processReportPath:"",createdAt:"2026-09-01T14:00:00",updatedAt:"2026-09-01T14:00:00"};

function mountCockpit(){return mount(CockpitScreensaver,{props:{quota:{available:true,freshness:"fresh",selectionReason:"test",primary:{usedPercent:32,remainingPercent:68,windowMinutes:300,resetsAt:0},secondary:{usedPercent:17,remainingPercent:83,windowMinutes:10080,resetsAt:0}},notifications,runningProjects:[runningProject],testRuns:[runningTest],tapdJobs:[runningTapd]}});}

describe("CockpitScreensaver",()=>{
  beforeEach(()=>{vi.useFakeTimers();vi.setSystemTime(new Date(2026,8,1,14,32,8));mocks.getDailyActivity.mockClear();});

  it("完整展示顶部统计、九十天热力和实时信息",async()=>{
    const wrapper=mountCockpit();await flushPromises();
    expect(wrapper.findAll(".cockpit-kpis article")).toHaveLength(6);
    expect(wrapper.findAll(".cockpit-kpis .cockpit-icon")).toHaveLength(6);
    expect(wrapper.find(".cockpit-kpis").text()).toContain("报告数量18");
    expect(wrapper.findAll(".heat-grid>i")).toHaveLength(90);
    expect(mocks.getDailyActivity).toHaveBeenCalledWith("2026-06-04","2026-09-01");
    expect(wrapper.find(".cockpit-quota-summary").text()).toContain("5h68%7d83%");
    expect(wrapper.find(".quota-panel").exists()).toBe(false);
    expect(wrapper.findAll(".messages-panel>div>button")).toHaveLength(3);
    expect(wrapper.findAll(".messages-panel .cockpit-icon")).toHaveLength(3);
    expect(wrapper.findAll(".running-panel>div>button")).toHaveLength(3);
    expect(wrapper.find(".running-panel").text()).toContain("项目 · 个人工作台");
    expect(wrapper.find(".running-panel").text()).toContain("测试 · 接口回归");
    expect(wrapper.find(".running-panel").text()).toContain("自动处理 · TAPD #415");
    expect(wrapper.text()).not.toContain("视频");
    wrapper.unmount();vi.useRealTimers();
  });

  it("没有运行任务时不渲染虚假的项目、测试和自动处理行",async()=>{
    const wrapper=mount(CockpitScreensaver,{props:{quota:{available:false,freshness:"",selectionReason:""},notifications:[],runningProjects:[],testRuns:[],tapdJobs:[]}});await flushPromises();
    expect(wrapper.findAll(".running-panel>div>button")).toHaveLength(0);
    expect(wrapper.find(".running-panel").text()).toContain("当前没有运行中的任务");
    expect(wrapper.find(".running-panel").text()).not.toContain("暂无运行");
    wrapper.unmount();vi.useRealTimers();
  });

  it("返回按钮和实时项通过事件交给壳层恢复或跳转",async()=>{
    const wrapper=mountCockpit();await flushPromises();
    await wrapper.get(".cockpit-return").trigger("click");
    expect(wrapper.emitted("close")).toHaveLength(1);
    await wrapper.get(".messages-panel>div>button").trigger("click");
    expect(wrapper.emitted("navigate")?.[0]).toEqual(["/inbox?item=0"]);
    wrapper.unmount();vi.useRealTimers();
  });
});
