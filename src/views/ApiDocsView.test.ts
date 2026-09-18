import { flushPromises, mount } from "@vue/test-utils";
import { createMemoryHistory, createRouter } from "vue-router";
// @ts-expect-error Vitest 在 Node 环境运行；前端生产类型配置无需引入 Node 全局类型。
import { readFileSync } from "node:fs";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ApiDocsView from "./ApiDocsView.vue";

declare const process: { cwd(): string };

const mocks = vi.hoisted(() => {
  const detailResult=(id:string) => ({ id,sourceId:id==="ep-3" ? "source-2":"source-1",operationId:id==="ep-2" ? "deleteUser":"getUser",method:id==="ep-2" ? "DELETE":"GET",path:id==="ep-3" ? "/orders":"/users/{id}",title:id==="ep-2" ? "删除用户":id==="ep-3" ? "查询订单":"查询用户",description:"按编号查询",tags:[id==="ep-2" ? "用户/管理":id==="ep-3" ? "订单":"用户"],deprecated:id==="ep-2",updatedAt:"",externalProjectId:id==="ep-3" ? "1002":"1001",apifoxProjectName:id==="ep-3" ? "订单接口":"客户接口",documentTitle:"示例服务",openapiVersion:"3.1.0",lastSyncedAt:"2026-08-31T08:00:00Z",document:{parameters:[{name:"id",in:"path",required:true,schema:{type:"string"}}],responses:{200:{description:"成功"}}} });
  return {
    detailResult,
    detail:vi.fn(async (id:string)=>detailResult(id)),
    render: vi.fn(async () => "# 查询用户\n\n`GET /users/{id}`"),
    renderCode: vi.fn(async () => "/**\n *  查询用户\n */\nexport function _getUser(params) {}"),
    saveSource: vi.fn(async (value) => ({id:value.id || "source-2",externalProjectId:value.externalProjectId,apifoxProjectName:value.apifoxProjectName,documentTitle:"",openapiVersion:"",syncStatus:"never",endpointCount:0,lastError:"",createdAt:"",updatedAt:""})),
    preview: vi.fn(async (id:string) => ({endpointId:id,url:"https://api.example.com/users/1",urlTemplate:"https://api.example.com/users/{id}",method:id==="ep-2" ? "DELETE":"GET",contentType:id==="ep-2" ? "application/json":"",headers:{Authorization:"[已配置，发送时安全注入]"},requestData:{"path.id":"1"},body:id==="ep-2" ? {name:"自动"}:null,requiresConfirmation:id==="ep-2",warning:id==="ep-2" ? "该请求可能修改数据":"请核对请求"})),
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
    { id:"ep-3",sourceId:"source-2",operationId:"getOrder",method:"GET",path:"/orders",title:"查询订单",description:"",tags:["订单"],deprecated:false,updatedAt:"",documentHash:"hash-1" },
  ] : [
    { id:"ep-1",sourceId:"source-1",operationId:"getUser",method:"GET",path:"/users/{id}",title:"查询用户",description:"按编号查询",tags:["用户"],deprecated:false,updatedAt:"",documentHash:"hash-1" },
    { id:"ep-2",sourceId:"source-1",operationId:"deleteUser",method:"DELETE",path:"/users/{id}",title:"删除用户",description:"",tags:["用户/管理"],deprecated:true,updatedAt:"",documentHash:"hash-1" },
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
    localStorage.clear();
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

  it("目录显示持久化更新提示，搜索展开不清除，手动展开后清除", async () => {
    localStorage.setItem("workbench-api-doc-updates-v1:source-1",JSON.stringify({known:{"ep-1":"old","ep-2":"old"},unread:{}}));
    const router=createRouter({history:createMemoryHistory(),routes:[{path:"/api-docs",component:ApiDocsView}]});
    await router.push("/api-docs");await router.isReady();
    const wrapper=mount(ApiDocsView,{global:{plugins:[router]}});await flushPromises();
    expect(wrapper.get(".api-tree-group").text()).toContain("有更新");
    expect(wrapper.get(".api-tree-toggle").attributes("aria-expanded")).toBe("false");
    await wrapper.get('input[placeholder*="搜索名称"]').setValue("用户");await flushPromises();
    expect(wrapper.findAll(".api-update-badge").length).toBeGreaterThan(0);
    await wrapper.get('input[placeholder*="搜索名称"]').setValue("");
    await wrapper.get(".api-tree-toggle").trigger("click");
    expect(wrapper.findAll(".api-update-badge")).toHaveLength(0);
    const saved=JSON.parse(localStorage.getItem("workbench-api-doc-updates-v1:source-1")!);
    expect(saved.unread).toEqual({});
    wrapper.unmount();
    const reopened=mount(ApiDocsView,{global:{plugins:[router]}});await flushPromises();
    expect(reopened.findAll(".api-update-badge")).toHaveLength(0);
    reopened.unmount();
  });

  it("大字号模式保持接口树和详情两栏布局", () => {
    const styles=readFileSync(`${process.cwd()}/src/styles/typography.css`,"utf8");
    expect(styles).toContain('.api-docs-layout { grid-template-columns: minmax(270px, 300px) minmax(0, 1fr); overflow-x: hidden; }');
    expect(styles).not.toContain(".api-docs-layout.source-collapsed");
  });

  it("按 Apifox 项目展示树形接口、可在项目名称下拉切换并复制接口内容", async () => {
    const router=createRouter({history:createMemoryHistory(),routes:[{path:"/api-docs",component:ApiDocsView},{path:"/settings",component:{template:"<div />"}}]});
    await router.push("/api-docs"); await router.isReady();
    const wrapper=mount(ApiDocsView,{global:{plugins:[router]}});
    await flushPromises();

    expect(wrapper.find(".api-source-panel").exists()).toBe(false);
    const projectSwitch=wrapper.get(".api-project-switch-trigger");
    expect(projectSwitch.text()).toContain("客户接口");
    expect(wrapper.find(".api-project-switch select").exists()).toBe(false);
    await projectSwitch.trigger("click");
    expect(projectSwitch.attributes("aria-expanded")).toBe("true");
    expect(wrapper.get(".api-project-switch-menu").text()).toContain("订单接口");
    expect(wrapper.text()).toContain("新增 Apifox 项目");
    expect(wrapper.text()).toContain("查询用户");
    expect(wrapper.find(".api-tree-group").text()).toContain("用户");
    expect(wrapper.text()).not.toContain("全部方法");
    expect(wrapper.text()).not.toContain("全部目录 / 标签");
    expect(wrapper.find(".api-tree-folder").exists()).toBe(false);
    expect(wrapper.text()).not.toContain("收藏");
    expect(wrapper.find(".api-endpoint-panel>header").text()).toContain("更新于");
    expect(wrapper.find(".api-endpoint-panel>header").text()).not.toContain("OpenAPI");
    await wrapper.findAll(".api-project-switch-menu>button").find(button=>button.text().includes("订单接口"))!.trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("查询订单");
    expect(wrapper.find(".api-project-switch-menu").exists()).toBe(false);
    await projectSwitch.trigger("click");
    await wrapper.findAll(".api-project-switch-menu>button").find(button=>button.text().includes("客户接口"))!.trigger("click");
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
    await wrapper.get('input[aria-label="选择 path.id"]').setValue(true);
    await wrapper.get('input[aria-label="参数值 path.id"]').setValue("1");
    await wrapper.findAll("button").find(button=>button.text().includes("发送 GET 请求"))!.trigger("click");
    await flushPromises();
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("/users/{id}");
    expect(mocks.execute).toHaveBeenCalledWith({endpointId:"ep-1",url:"https://api.example.com/users/1",body:null,parameters:[{name:"id",location:"path",value:"1"}],confirmed:false});
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

  it.each([false, true])("HTML 响应按文档声明决定是否提示异常（已声明：%s）", async (declaredHtml) => {
    if (declaredHtml) mocks.detail.mockImplementationOnce(async (id:string) => {
      const detail = mocks.detailResult(id);
      Object.assign(detail.document.responses[200], {content:{"text/html":{}}});
      return detail;
    });
    mocks.execute.mockResolvedValueOnce({url:"https://api.example.com/users/1",method:"GET",status:200,statusText:"OK",success:true,elapsedMs:18,contentType:"text/html; charset=utf-8",requestData:{"path.id":"1"},responseData:"<!DOCTYPE html><html>前端页面</html>" as never,truncated:false});
    const router=createRouter({history:createMemoryHistory(),routes:[{path:"/api-docs",component:ApiDocsView}]});
    await router.push("/api-docs");await router.isReady();
    const wrapper=mount(ApiDocsView,{global:{plugins:[router]}});await flushPromises();
    await wrapper.findAll("button").find(button=>button.text().includes("接口测试"))!.trigger("click");await flushPromises();
    await wrapper.get('input[aria-label="选择 path.id"]').setValue(true);
    await wrapper.get('input[aria-label="参数值 path.id"]').setValue("1");
    await wrapper.findAll("button").find(button=>button.text().includes("发送 GET 请求"))!.trigger("click");await flushPromises();
    expect(wrapper.get(".api-test-result").classes()).toContain(declaredHtml ? "success" : "error");
    if (!declaredHtml) expect(wrapper.get('[role="alert"]').text()).toContain("请求基地址是否包含 API 前缀");
    else expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    expect(wrapper.text()).toContain("前端页面");
    wrapper.unmount();
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
    await wrapper.get('input[aria-label="选择 path.id"]').setValue(true);
    await wrapper.get('input[aria-label="参数值 path.id"]').setValue("");
    await send.trigger("click");
    expect(wrapper.text()).toContain("请填写必填参数 id");
    expect(mocks.execute).not.toHaveBeenCalled();
    await wrapper.get('input[aria-label="参数值 path.id"]').setValue("1");
    await send.trigger("click");await flushPromises();
    expect(mocks.execute).toHaveBeenCalledWith({endpointId:"ep-2",url:"https://api.example.com/users/1",body:null,parameters:[{name:"id",location:"path",value:"1"}],confirmed:true});
  });

  it("必填锁定并自动填值、分页默认填入，取消全选保留必填项", async () => {
    mocks.detail.mockImplementationOnce(async (id:string)=>{
      const detail=mocks.detailResult(id);
      detail.document.parameters.push({name:"pageSize",in:"query",required:false,schema:{type:"integer"}},{name:"enabled",in:"query",required:false,schema:{type:"boolean"}});
      Object.assign(detail.document,{requestBody:{content:{"application/json":{schema:{type:"object",properties:{filter:{type:"object"}}}}}}});
      return detail;
    });
    const router=createRouter({history:createMemoryHistory(),routes:[{path:"/api-docs",component:ApiDocsView}]});
    await router.push("/api-docs");await router.isReady();
    const wrapper=mount(ApiDocsView,{global:{plugins:[router]}});await flushPromises();
    await wrapper.findAll("button").find(button=>button.text().includes("接口测试"))!.trigger("click");await flushPromises();
    expect((wrapper.get('input[aria-label="选择 path.id"]').element as HTMLInputElement).checked).toBe(true);
    expect(wrapper.get('input[aria-label="选择 path.id"]').attributes('disabled')).toBeDefined();
    expect((wrapper.get('input[aria-label="参数值 path.id"]').element as HTMLInputElement).value).toBe("1");
    expect((wrapper.get('input[aria-label="选择 query.pageSize"]').element as HTMLInputElement).checked).toBe(true);
    expect((wrapper.get('input[aria-label="参数值 query.pageSize"]').element as HTMLInputElement).value).toBe("10");
    const send=wrapper.findAll("button").find(button=>button.text().includes("发送 GET 请求"))!;
    await wrapper.get('input[aria-label="参数值 path.id"]').setValue("");
    await send.trigger("click");expect(wrapper.text()).toContain("请填写必填参数 id");
    await wrapper.get('.api-parameter-select-all input').setValue(true);
    await wrapper.get('input[aria-label="参数值 path.id"]').setValue("2");
    await wrapper.get('input[aria-label="参数值 query.pageSize"]').setValue("20");
    await wrapper.get('input[aria-label="参数值 query.enabled"]').setValue("false");
    await wrapper.get('textarea[aria-label="参数值 body.filter"]').setValue('{"name":"会议"}');
    await wrapper.get('input[aria-label="选择 query.pageSize"]').setValue(false);
    await send.trigger("click");await flushPromises();
    expect(mocks.execute).toHaveBeenCalledWith(expect.objectContaining({url:"https://api.example.com/users/2?enabled=false",parameters:[{name:"id",location:"path",value:"2"},{name:"enabled",location:"query",value:false},{name:"filter",location:"body",value:{name:"会议"}}]}));
    await wrapper.findAll("button").find(button=>button.text().includes("接口测试"))!.trigger("click");await flushPromises();
    expect((wrapper.get('input[aria-label="选择 path.id"]').element as HTMLInputElement).checked).toBe(true);
    expect((wrapper.get('input[aria-label="选择 query.enabled"]').element as HTMLInputElement).checked).toBe(false);
    await wrapper.get('.api-parameter-select-all input').setValue(true);
    await wrapper.get('.api-parameter-select-all input').setValue(false);
    expect((wrapper.get('input[aria-label="选择 path.id"]').element as HTMLInputElement).checked).toBe(true);
    expect((wrapper.get('input[aria-label="选择 query.pageSize"]').element as HTMLInputElement).checked).toBe(false);
    wrapper.unmount();
  });

  it("可选参数勾选后允许为空或不匹配类型，只有必填参数校验", async () => {
    mocks.detail.mockImplementationOnce(async (id:string)=>{
      const detail=mocks.detailResult(id);
      detail.document.parameters.push({name:"count",in:"query",required:false,schema:{type:"integer"}},{name:"enabled",in:"query",required:false,schema:{type:"boolean"}},{name:"requiredCount",in:"query",required:true,schema:{type:"integer"}});
      Object.assign(detail.document,{requestBody:{content:{"application/json":{schema:{type:"object",properties:{filter:{type:"object"}}}}}}});
      return detail;
    });
    const router=createRouter({history:createMemoryHistory(),routes:[{path:"/api-docs",component:ApiDocsView}]});
    await router.push("/api-docs");await router.isReady();
    const wrapper=mount(ApiDocsView,{global:{plugins:[router]}});await flushPromises();
    await wrapper.findAll("button").find(button=>button.text().includes("接口测试"))!.trigger("click");await flushPromises();
    await wrapper.get('.api-parameter-select-all input').setValue(true);
    await wrapper.get('input[aria-label="参数值 query.enabled"]').setValue("anything");
    await wrapper.get('input[aria-label="参数值 query.requiredCount"]').setValue("invalid");
    const send=wrapper.findAll("button").find(button=>button.text().includes("发送 GET 请求"))!;
    await send.trigger("click");expect(wrapper.text()).toContain("不是有效的 integer");
    expect(mocks.execute).not.toHaveBeenCalled();
    await wrapper.get('input[aria-label="参数值 query.requiredCount"]').setValue("0");
    await send.trigger("click");await flushPromises();
    expect(mocks.execute).toHaveBeenCalledWith(expect.objectContaining({parameters:[{name:"id",location:"path",value:"1"},{name:"count",location:"query",value:""},{name:"enabled",location:"query",value:"anything"},{name:"requiredCount",location:"query",value:0},{name:"filter",location:"body",value:""}]}));
    wrapper.unmount();
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
