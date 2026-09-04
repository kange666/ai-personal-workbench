import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ApiDocsView from "./ApiDocsView.vue";

const mocks = vi.hoisted(() => {
  const detailResult=(id:string) => ({ id,sourceId:id==="ep-3" ? "source-2":"source-1",operationId:id==="ep-2" ? "deleteUser":"getUser",method:id==="ep-2" ? "DELETE":"GET",path:id==="ep-3" ? "/orders":"/users/{id}",title:id==="ep-2" ? "删除用户":id==="ep-3" ? "查询订单":"查询用户",description:"按编号查询",tags:[id==="ep-2" ? "用户/管理":id==="ep-3" ? "订单":"用户"],deprecated:id==="ep-2",updatedAt:"",externalProjectId:id==="ep-3" ? "1002":"1001",apifoxProjectName:id==="ep-3" ? "订单接口":"客户接口",documentTitle:"示例服务",openapiVersion:"3.1.0",lastSyncedAt:"2026-08-31T08:00:00Z",document:{parameters:[{name:"id",in:"path",required:true,schema:{type:"string"}}],responses:{200:{description:"成功"}}} });
  return {
    detailResult,
    detail:vi.fn(async (id:string)=>detailResult(id)),
    render: vi.fn(async () => "# 查询用户\n\n`GET /users/{id}`"),
    renderCode: vi.fn(async () => "/**\n *  查询用户\n */\nexport function _getUser(params) {}"),
    saveSource: vi.fn(async (value) => ({id:value.id || "source-2",externalProjectId:value.externalProjectId,apifoxProjectName:value.apifoxProjectName,documentTitle:"",openapiVersion:"",syncStatus:"never",endpointCount:0,lastError:"",createdAt:"",updatedAt:""})),
    preview: vi.fn(async (id:string) => ({endpointId:id,url:"https://api.example.com/users/1",method:id==="ep-2" ? "DELETE":"GET",contentType:id==="ep-2" ? "application/json":"",headers:{Authorization:"[已配置，发送时安全注入]"},requestData:{"path.id":"1"},body:id==="ep-2" ? {name:"自动"}:null,requiresConfirmation:id==="ep-2",warning:id==="ep-2" ? "该请求可能修改数据":"请核对请求"})),
    execute: vi.fn(async () => ({url:"https://api.example.com/users/1",method:"GET",status:200,statusText:"OK",success:true,elapsedMs:18,contentType:"application/json",requestData:{"path.id":"1"},responseData:{code:200,data:{id:"1"}},truncated:false})),
    saveTemplate: vi.fn(async (value)=>value),
    saveTestConfig: vi.fn(async (value)=>({...value,tokenConfigured:Boolean(value.token)})),
  };
});

vi.mock("../services/backend", () => ({
  isTauriRuntime: () => true,
  getApifoxCredentialStatus: async () => ({ configured:true,source:"Windows 凭据库" }),
  listApiSources: async () => [{ id:"source-1",externalProjectId:"1001",apifoxProjectName:"客户接口",documentTitle:"示例服务",openapiVersion:"3.1.0",syncStatus:"ready",endpointCount:2,lastSyncedAt:"2026-08-31T08:00:00Z",lastError:"",createdAt:"",updatedAt:"" },{ id:"source-2",externalProjectId:"1002",apifoxProjectName:"订单接口",documentTitle:"订单服务",openapiVersion:"3.1.0",syncStatus:"ready",endpointCount:1,lastSyncedAt:"2026-08-31T08:00:00Z",lastError:"",createdAt:"",updatedAt:"" }],
  listApiEndpoints: async (sourceId:string) => sourceId==="source-2" ? [
    { id:"ep-3",sourceId:"source-2",operationId:"getOrder",method:"GET",path:"/orders",title:"查询订单",description:"",tags:["订单"],deprecated:false,updatedAt:"" },
  ] : [
    { id:"ep-1",sourceId:"source-1",operationId:"getUser",method:"GET",path:"/users/{id}",title:"查询用户",description:"按编号查询",tags:["用户"],deprecated:false,updatedAt:"" },
    { id:"ep-2",sourceId:"source-1",operationId:"deleteUser",method:"DELETE",path:"/users/{id}",title:"删除用户",description:"",tags:["用户/管理"],deprecated:true,updatedAt:"" },
  ],
  getApiEndpoint: mocks.detail,
  renderApiEndpointMarkdown: mocks.render,
  renderApiEndpointRequestCode: mocks.renderCode,
  getApiTagExport: async () => ({sourceId:"source-1",tagPath:"用户",openapiUrl:"http://127.0.0.1:17890/openapi/source-1/abc.json?version=3.0",endpointCount:2,available:true}),
  previewApiEndpointTest: mocks.preview,
  executeApiEndpointTest: mocks.execute,
  getApiCodeTemplate: async () => ({sourceId:"source-1",client:"request",functionPrefix:"_",importPath:"",includeImport:false,typescript:false}),
  saveApiCodeTemplate: mocks.saveTemplate,
  getApiTestConfig: async () => ({sourceId:"source-1",baseUrl:"https://api.example.com",tokenHeader:"Authorization",tokenConfigured:true}),
  saveApiTestConfig: mocks.saveTestConfig, clearApiTestToken: vi.fn(),
  saveApiSource: mocks.saveSource, removeApiSource: vi.fn(), syncApiSource: vi.fn(), syncAllApiSources: vi.fn(),
}));

describe("ApiDocsView", () => {
  beforeEach(() => {
    mocks.render.mockClear();
    mocks.renderCode.mockClear();
    mocks.detail.mockReset();mocks.detail.mockImplementation(async (id:string)=>mocks.detailResult(id));
    mocks.saveSource.mockClear();
    mocks.preview.mockClear();
    mocks.execute.mockClear();
    mocks.saveTemplate.mockClear();
    mocks.saveTestConfig.mockClear();
    Object.defineProperty(navigator,"clipboard",{ configurable:true,value:{ writeText:vi.fn(async()=>undefined) } });
  });

  it("按 Apifox 项目展示树形接口、可在项目名称下拉切换并复制接口内容", async () => {
    const router=createRouter({history:createMemoryHistory(),routes:[{path:"/api-docs",component:ApiDocsView},{path:"/settings",component:{template:"<div />"}}]});
    await router.push("/api-docs"); await router.isReady();
    const wrapper=mount(ApiDocsView,{global:{plugins:[router]}});
    await flushPromises();

    expect(wrapper.find(".api-source-panel").exists()).toBe(false);
    const projectSwitch=wrapper.get(".api-project-switch select");
    expect(projectSwitch.text()).toContain("客户接口");
    expect(projectSwitch.text()).toContain("订单接口");
    expect(wrapper.text()).toContain("新增 Apifox 项目");
    expect(wrapper.text()).toContain("查询用户");
    expect(wrapper.find(".api-tree-group").text()).toContain("用户");
    expect(wrapper.text()).not.toContain("全部方法");
    expect(wrapper.text()).not.toContain("全部目录 / 标签");
    expect(wrapper.find(".api-tree-folder").exists()).toBe(false);
    expect(wrapper.text()).not.toContain("收藏");
    expect(wrapper.find(".api-endpoint-panel>header").text()).toContain("更新于");
    expect(wrapper.find(".api-endpoint-panel>header").text()).not.toContain("OpenAPI");
    await projectSwitch.setValue("source-2");
    await flushPromises();
    expect(wrapper.text()).toContain("查询订单");
    await projectSwitch.setValue("source-1");
    await flushPromises();
    const search=wrapper.get('input[placeholder*="搜索名称"]');
    await search.setValue("删除");
    expect(wrapper.findAll(".api-tree-endpoint")).toHaveLength(1);
    expect(wrapper.find(".api-tree-endpoint").text()).toContain("删除用户");
    expect(wrapper.findAll(".api-tree-group").map(item=>item.text()).join(" ")).toContain("管理");
    await search.setValue("");
    await flushPromises();
    await wrapper.find(".api-tag-export-trigger").trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("标签 OpenAPI 导出");
    await wrapper.findAll("button").find(button=>button.text().includes("复制 URL"))!.trigger("click");
    await wrapper.get(".api-tag-export-editor .icon-button").trigger("click");
    await wrapper.get(".api-path-copy").trigger("click");
    await wrapper.findAll("button").find(button=>button.text().includes("接口测试"))!.trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("确认接口测试请求");
    expect(mocks.execute).not.toHaveBeenCalled();
    await wrapper.findAll("button").find(button=>button.text().includes("发送 GET 请求"))!.trigger("click");
    await flushPromises();
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("/users/{id}");
    expect(mocks.execute).toHaveBeenCalledWith({endpointId:"ep-1",url:"https://api.example.com/users/1",body:null,confirmed:false});
    expect(wrapper.text()).toContain("HTTP 200");
    expect(wrapper.text()).toContain('"code": 200');
    await wrapper.findAll("button").find(button=>button.text().includes("复制接口代码"))!.trigger("click");
    await flushPromises();
    expect(mocks.renderCode).toHaveBeenCalledWith("ep-1");
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("/**\n *  查询用户\n */\nexport function _getUser(params) {}");
    await wrapper.findAll("button").find(button=>button.text().includes("复制 Markdown"))!.trigger("click");
    await flushPromises();

    expect(mocks.render).toHaveBeenCalledWith("ep-1");
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("# 查询用户\n\n`GET /users/{id}`");
    expect(wrapper.text()).toContain("敏感示例值已遮盖");
  });

  it("写操作必须在预览中明确确认后才能发送", async () => {
    const router=createRouter({history:createMemoryHistory(),routes:[{path:"/api-docs",component:ApiDocsView},{path:"/settings",component:{template:"<div />"}}]});
    await router.push("/api-docs");await router.isReady();
    const wrapper=mount(ApiDocsView,{global:{plugins:[router]}});await flushPromises();
    await wrapper.get('input[placeholder*="搜索名称"]').setValue("删除");
    await wrapper.get(".api-tree-endpoint").trigger("click");await flushPromises();
    await wrapper.findAll("button").find(button=>button.text().includes("接口测试"))!.trigger("click");await flushPromises();
    const send=wrapper.findAll("button").find(button=>button.text().includes("发送 DELETE 请求"))!;
    expect(send.attributes("disabled")).toBeDefined();
    expect(mocks.execute).not.toHaveBeenCalled();
    await wrapper.get('.api-danger-confirm input[type="checkbox"]').setValue(true);
    await wrapper.get(".api-test-preview-editor textarea").setValue("{invalid");
    await send.trigger("click");
    expect(wrapper.text()).toContain("请求体不是有效的 JSON");
    expect(mocks.execute).not.toHaveBeenCalled();
    await wrapper.get(".api-test-preview-editor textarea").setValue("");
    await send.trigger("click");await flushPromises();
    expect(mocks.execute).toHaveBeenCalledWith({endpointId:"ep-2",url:"https://api.example.com/users/1",body:null,confirmed:true});
  });

  it("可以不关联规范项目直接新增 Apifox 项目并保存统一配置", async () => {
    const router=createRouter({history:createMemoryHistory(),routes:[{path:"/api-docs",component:ApiDocsView},{path:"/settings",component:{template:"<div />"}}]});
    await router.push("/api-docs");await router.isReady();
    const wrapper=mount(ApiDocsView,{global:{plugins:[router]}});await flushPromises();
    await wrapper.findAll("button").find(button=>button.text().includes("新增 Apifox 项目"))!.trigger("click");await flushPromises();
    expect(wrapper.find(".api-project-settings-editor").classes()).toContain("api-project-settings-editor");
    expect(wrapper.text()).toContain("Apifox 项目");
    expect(wrapper.text()).toContain("接口测试");
    expect(wrapper.text()).toContain("复制代码模板");
    expect(wrapper.find(".api-profile-picker").exists()).toBe(false);
    await wrapper.get('.api-project-settings-editor input[placeholder="例如 client 接口项目"]').setValue("统一接口项目");
    await wrapper.get('.api-project-settings-editor input[placeholder*="基本设置中复制"]').setValue("2002");
    await wrapper.get(".api-project-settings-editor select").setValue("axios");
    await wrapper.get('.api-project-settings-editor input[placeholder="例如 _ 或 api"]').setValue("api");
    await wrapper.findAll("button").find(button=>button.text()==="新增项目")!.trigger("click");await flushPromises();
    expect(mocks.saveSource).toHaveBeenCalledWith({id:"",externalProjectId:"2002",apifoxProjectName:"统一接口项目"});
    expect(mocks.saveTestConfig).toHaveBeenCalledTimes(1);
    expect(mocks.saveTemplate).toHaveBeenCalledTimes(1);
    expect(mocks.saveTemplate).toHaveBeenCalledWith(expect.objectContaining({sourceId:"source-2",client:"axios",functionPrefix:"api"}));
  });

  it("切换接口时只在详情面板显示加载状态", async () => {
    const router=createRouter({history:createMemoryHistory(),routes:[{path:"/api-docs",component:ApiDocsView},{path:"/settings",component:{template:"<div />"}}]});
    await router.push("/api-docs");await router.isReady();
    const wrapper=mount(ApiDocsView,{global:{plugins:[router]}});
    await flushPromises();
    let release=()=>{};
    mocks.detail.mockImplementationOnce((id:string)=>new Promise(resolve=>{release=()=>resolve(mocks.detailResult(id));}));
    await wrapper.find(".api-tree-endpoint").trigger("click");
    expect(wrapper.find(".api-detail-loading").exists()).toBe(true);
    expect(wrapper.find(".api-detail-loading").text()).toContain("正在加载接口详情");
    release();await flushPromises();
    expect(wrapper.find(".api-detail-loading").exists()).toBe(false);
  });
});
