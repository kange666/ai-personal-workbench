import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ApiDocsView from "./ApiDocsView.vue";

const mocks = vi.hoisted(() => ({
  render: vi.fn(async () => "# 查询用户\n\n`GET /users/{id}`"),
  saveKnowledge: vi.fn(async () => ({ id:"api:ep-1" })),
}));

vi.mock("../services/backend", () => ({
  isTauriRuntime: () => true,
  getApifoxCredentialStatus: async () => ({ configured:true,source:"Windows 凭据库" }),
  listProjectProfiles: async () => [{ id:"project-1",displayName:"示例项目",repositoryPath:"F:/project",tapdWorkspaceId:"",aliases:[],category:"",createdAt:"",updatedAt:"" }],
  listApiSources: async () => [{ id:"source-1",projectProfileId:"project-1",projectName:"示例项目",repositoryPath:"F:/project",externalProjectId:"1001",documentTitle:"示例服务",openapiVersion:"3.1.0",syncStatus:"ready",endpointCount:2,lastSyncedAt:"2026-08-31T08:00:00Z",lastError:"",createdAt:"",updatedAt:"" }],
  listApiEndpoints: async () => [
    { id:"ep-1",sourceId:"source-1",operationId:"getUser",method:"GET",path:"/users/{id}",title:"查询用户",description:"按编号查询",tags:["用户"],deprecated:false,updatedAt:"" },
    { id:"ep-2",sourceId:"source-1",operationId:"deleteUser",method:"DELETE",path:"/users/{id}",title:"删除用户",description:"",tags:["用户"],deprecated:true,updatedAt:"" },
  ],
  getApiEndpoint: async (id:string) => ({ id,sourceId:"source-1",operationId:"getUser",method:"GET",path:"/users/{id}",title:"查询用户",description:"按编号查询",tags:["用户"],deprecated:false,updatedAt:"",projectProfileId:"project-1",projectName:"示例项目",repositoryPath:"F:/project",documentTitle:"示例服务",openapiVersion:"3.1.0",lastSyncedAt:"2026-08-31T08:00:00Z",document:{parameters:[{name:"id",in:"path",required:true,schema:{type:"string"}}],responses:{200:{description:"成功"}}} }),
  renderApiEndpointMarkdown: mocks.render,
  saveApiEndpointToKnowledge: mocks.saveKnowledge,
  saveApiSource: vi.fn(), removeApiSource: vi.fn(), syncApiSource: vi.fn(), syncAllApiSources: vi.fn(),
}));

describe("ApiDocsView", () => {
  beforeEach(() => {
    mocks.render.mockClear();
    mocks.saveKnowledge.mockClear();
    Object.defineProperty(navigator,"clipboard",{ configurable:true,value:{ writeText:vi.fn(async()=>undefined) } });
  });

  it("按项目展示接口、筛选并复制后端生成的 Markdown", async () => {
    const router=createRouter({history:createMemoryHistory(),routes:[{path:"/api-docs",component:ApiDocsView},{path:"/knowledge",component:{template:"<div />"}},{path:"/testing",component:{template:"<div />"}},{path:"/settings",component:{template:"<div />"}}]});
    await router.push("/api-docs"); await router.isReady();
    const wrapper=mount(ApiDocsView,{global:{plugins:[router]}});
    await flushPromises();

    expect(wrapper.text()).toContain("示例项目");
    expect(wrapper.text()).toContain("查询用户");
    const search=wrapper.get('input[placeholder*="搜索名称"]');
    await search.setValue("删除");
    expect(wrapper.findAll(".api-endpoint-list>button")).toHaveLength(1);
    expect(wrapper.find(".api-endpoint-list>button").text()).toContain("删除用户");
    await search.setValue("");
    await flushPromises();
    await wrapper.findAll("button").find(button=>button.text().includes("复制 Markdown"))!.trigger("click");
    await flushPromises();

    expect(mocks.render).toHaveBeenCalledWith("ep-1");
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("# 查询用户\n\n`GET /users/{id}`");
    expect(wrapper.text()).toContain("敏感示例值已遮盖");
  });
});
