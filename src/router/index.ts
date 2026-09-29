import { createRouter, createWebHashHistory } from "vue-router";
const DashboardView = () => import("../views/DashboardView.vue");
const CalendarView = () => import("../views/CalendarView.vue");
const ReportsView = () => import("../views/ReportsView.vue");
const TokensView = () => import("../views/TokensView.vue");
const KnowledgeView = () => import("../views/KnowledgeView.vue");
const ContentView = () => import("../views/ContentView.vue");
const SettingsView = () => import("../views/SettingsView.vue");
const TapdView = () => import("../views/TapdView.vue");
const TapdAutomationView = () => import("../views/TapdAutomationView.vue");
const TestingView = () => import("../views/TestingView.vue");
const WorkRecordsView = () => import("../views/WorkRecordsView.vue");
const VideoCenterView = () => import("../views/VideoCenterView.vue");
const ProjectsView = () => import("../views/ProjectsView.vue");
const DeploymentsView = () => import("../views/DeploymentsView.vue");
const InboxView = () => import("../views/InboxView.vue");
const ProjectMappingView = () => import("../views/ProjectMappingView.vue");
const ApiDocsView = () => import("../views/ApiDocsView.vue");
import { getVipStatus, isTauriRuntime } from "../services/backend";
import { APP_BRAND } from "../utils/brand";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", name: "dashboard", component: DashboardView, meta: { title: "工作台" } },
    { path: "/rest", name: "rest", component: () => import("../views/RestView.vue"), meta: { title: "休息站" } },
    { path: "/work-records", name: "work-records", component: WorkRecordsView, meta: { title: "工作记录" } },
    { path: "/projects", name: "projects", component: ProjectsView, meta: { title: "项目资产" } },
    { path: "/deployments", name: "deployments", component: DeploymentsView, meta: { title: "发布中心" } },
    { path: "/project-mapping", name: "project-mapping", component: ProjectMappingView, meta: { title: "项目身份映射" } },
    { path: "/api-docs", name: "api-docs", component: ApiDocsView, meta: { title: "接口文档中心" } },
    { path: "/inbox", name: "inbox", component: InboxView, meta: { title: "待处理收件箱" } },
    { path: "/tasks", redirect: (to) => ({ path: "/calendar", query: { ...to.query, tab: "tasks" } }) },
    { path: "/calendar", name: "calendar", component: CalendarView, meta: { title: "工作日历" } },
    { path: "/reports", name: "reports", component: ReportsView, meta: { title: "报告中心" } },
    { path: "/tokens", name: "tokens", component: TokensView, meta: { title: "Token 分析" } },
    { path: "/knowledge", name: "knowledge", component: KnowledgeView, meta: { title: "知识库" } },
    { path: "/content", name: "content", component: ContentView, meta: { title: "内容工坊", requiresVip: true } },
    { path: "/videos", name: "videos", component: VideoCenterView, meta: { title: "视频中心", requiresVip: true } },
    { path: "/testing", name: "testing", component: TestingView, meta: { title: "测试中心" } },
    { path: "/tapd", name: "tapd", component: TapdView, meta: { title: "TAPD 工作" } },
    { path: "/tapd-automation", name: "tapd-automation", component: TapdAutomationView, meta: { title: "自动处理" } },
    { path: "/settings", name: "settings", component: SettingsView, meta: { title: "设置" } },
  ],
});

router.beforeEach(async (to) => {
  if (!to.meta.requiresVip) return true;
  if (!isTauriRuntime()) return { path: "/settings", query: { vip: "required" } };
  try {
    if ((await getVipStatus()).active) return true;
  } catch { /* 设置页会显示可操作的 VIP 入口 */ }
  return { path: "/settings", query: { vip: "required" } };
});

router.afterEach((to) => {
  document.title = `${String(to.meta.title)} · ${APP_BRAND.name}`;
});

export default router;
