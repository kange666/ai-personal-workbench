import { createRouter, createWebHashHistory } from "vue-router";
import DashboardView from "../views/DashboardView.vue";
import TasksView from "../views/TasksView.vue";
import CalendarView from "../views/CalendarView.vue";
import ReportsView from "../views/ReportsView.vue";
import TokensView from "../views/TokensView.vue";
import KnowledgeView from "../views/KnowledgeView.vue";
import ContentView from "../views/ContentView.vue";
import SettingsView from "../views/SettingsView.vue";
import TestingView from "../views/TestingView.vue";
import WorkRecordsView from "../views/WorkRecordsView.vue";
import VideoCenterView from "../views/VideoCenterView.vue";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", name: "dashboard", component: DashboardView, meta: { title: "工作台" } },
    { path: "/work-records", name: "work-records", component: WorkRecordsView, meta: { title: "工作记录" } },
    { path: "/tasks", name: "tasks", component: TasksView, meta: { title: "任务中心" } },
    { path: "/calendar", name: "calendar", component: CalendarView, meta: { title: "日历与甘特" } },
    { path: "/reports", name: "reports", component: ReportsView, meta: { title: "报告中心" } },
    { path: "/tokens", name: "tokens", component: TokensView, meta: { title: "Token 分析" } },
    { path: "/knowledge", name: "knowledge", component: KnowledgeView, meta: { title: "知识库" } },
    { path: "/content", name: "content", component: ContentView, meta: { title: "内容工坊" } },
    { path: "/videos", name: "videos", component: VideoCenterView, meta: { title: "视频中心" } },
    { path: "/testing", name: "testing", component: TestingView, meta: { title: "测试中心" } },
    { path: "/settings", name: "settings", component: SettingsView, meta: { title: "设置" } },
  ],
});

router.afterEach((to) => {
  document.title = `${String(to.meta.title)} · AI 个人工作台`;
});

export default router;
