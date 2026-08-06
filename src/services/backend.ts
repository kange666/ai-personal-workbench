import { invoke } from "@tauri-apps/api/core";
import type { WorkTask } from "../types/workbench";

export interface DatabaseHealth {
  path: string;
  schemaVersion: number;
}

export interface CodexScanSummary {
  filesScanned: number;
  normalFilesScanned: number;
  archivedFilesScanned: number;
  conversationsImported: number;
  tokenEventsImported: number;
  messagesImported: number;
  filesUnchanged: number;
  archivedConversationsImported: number;
  conversationsTotal: number;
  archivedConversationsTotal: number;
  errors: number;
  errorDetails: string[];
}

export interface GitScanConfiguration {
  roots: string[];
  maxDepth: number;
}

export interface CodexQuotaWindow {
  usedPercent: number;
  remainingPercent: number;
  windowMinutes: number;
  resetsAt: number;
}

export interface CodexQuotaSnapshot {
  available: boolean;
  capturedAt?: string;
  planType?: string;
  primary?: CodexQuotaWindow;
  secondary?: CodexQuotaWindow;
}

export interface GitScanSummary {
  repositoriesFound: number;
  commitsImported: number;
  snapshotsCreated: number;
  errors: number;
}

export interface RepositoryAsset {
  path: string;
  name: string;
  category: string;
  purpose: string;
  technologyStack: string;
  mainModules: string;
  installCommand: string;
  startCommand: string;
  testCommand: string;
  buildCommand: string;
  commandSource: string;
  remoteUrl: string;
  defaultBranch: string;
  hasUncommittedChanges: boolean;
  inferenceStatus: string;
  manuallyConfirmed: boolean;
  lastScannedAt: string;
  healthLevel: string;
  healthSummary: string;
  commitCount: number;
  conversationCount: number;
}

export interface RepositoryAssetUpdate {
  path: string;
  category: string;
  purpose: string;
  technologyStack: string;
  mainModules: string;
  installCommand: string;
  startCommand: string;
  testCommand: string;
  buildCommand: string;
  commandSource: string;
}

export interface RepositoryAssetDetails {
  conversations: Array<{ id: string; title: string; updatedAt: string; archived: boolean }>;
  commits: Array<{ hash: string; subject: string; committedAt: string }>;
}

export interface TokenSummary {
  conversationCount: number;
  messageCount: number;
  activeDays: number;
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
  totalTokens: number;
}

export interface ConversationMetric {
  id: string;
  title?: string;
  cwd?: string;
  project: string;
  model?: string;
  updatedAt?: string;
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
  totalTokens: number;
  contextUsedTokens: number;
  contextWindow: number;
  archived: boolean;
}

export interface TokenTrendPoint {
  date: string;
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
  totalTokens: number;
}

export interface ProjectTokenMetric {
  project: string;
  conversationCount: number;
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
  totalTokens: number;
}

export interface ModelTokenMetric {
  model: string;
  conversationCount: number;
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
  totalTokens: number;
}

export interface HistoricalReportSummary {
  activeDays: number;
  activeWeeks: number;
  dailyGenerated: number;
  weeklyGenerated: number;
  existingSkipped: number;
  dailyUpdated: number;
  weeklyUpdated: number;
  lockedSkipped: number;
  filesScanned: number;
  normalFilesScanned: number;
  archivedFilesScanned: number;
  conversationsTotal: number;
  archivedConversationsTotal: number;
  messagesTotal: number;
  firstDate?: string;
  lastDate?: string;
}

export interface HistoryCoverage {
  conversations: number;
  archivedConversations: number;
  messages: number;
  activeDays: number;
  activeWeeks: number;
  dailyReports: number;
  weeklyReports: number;
  firstDate?: string;
  lastDate?: string;
}

export interface SuggestionSyncSummary {
  conversationSuggestions: number;
  reportSuggestions: number;
  testSuggestions: number;
}

export interface DailyActivity {
  date: string;
  conversationCount: number;
  archivedConversationCount: number;
  messageCount: number;
  userMessages: number;
  assistantMessages: number;
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
  totalTokens: number;
  gitCommits: number;
  contentIdeaCount: number;
  dailyReportId?: string;
  weeklyReportId?: string;
  workMinutes: number;
  estimatedWorkMinutes: number;
  manualWorkMinutes: number;
  testRuns: number;
  testsPassed: number;
  knowledgeCount: number;
  taskActivityCount: number;
}

export interface WorkspaceSearchResult {
  id: string;
  kind: "任务" | "Codex 对话" | "报告" | "知识" | "内容";
  title: string;
  subtitle: string;
  date?: string;
  route: string;
}

export interface ContentIdea {
  id: string;
  ideaDate: string;
  contentType: "tech" | "reasoning";
  category: string;
  title: string;
  hook: string;
  script: string;
  storyboard: string;
  visualPrompts: string;
  editingGuide: string;
  coverTitle: string;
  status: "candidate" | "selected" | "rejected" | "published";
  source: "local" | "deepseek";
  createdAt: string;
  updatedAt: string;
}

export interface ReportRecord {
  id: string;
  reportType: "daily" | "weekly" | "monthly";
  periodStart: string;
  periodEnd: string;
  title: string;
  contentMarkdown: string;
  status: "draft" | "locked";
  createdAt: string;
  updatedAt: string;
}

export interface ReportSource {
  kind: "Codex 对话" | "Git 提交" | "任务" | "测试";
  id: string;
  title: string;
  project: string;
  date: string;
  detail: string;
}

export interface KnowledgeItem {
  id: string;
  kind: "decision" | "experience" | "risk" | "skill";
  title: string;
  content: string;
  project?: string;
  sourceType?: "manual" | "conversation" | "report";
  sourceId?: string;
  tags: string;
  confirmed: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface KnowledgeSyncSummary {
  conversationsScanned: number;
  itemsGenerated: number;
  decisions: number;
  experiences: number;
  risks: number;
  skills: number;
}

export interface AiStatus {
  configured: boolean;
  source: string;
  model: string;
}

export interface KnowledgeAnswer {
  answer: string;
  sources: Array<{ id: string; title: string; sourceType: string; sourceId?: string }>;
}

export interface TestCapabilities {
  mock: boolean;
  realApi: boolean;
  sourceStyle: boolean;
  browserStyle: boolean;
}

export interface TestMenu {
  id: string;
  project: "client" | "APP";
  name: string;
  route: string;
  sourcePath: string;
  caseId?: string;
  capabilities: TestCapabilities;
  tested: boolean;
  latestStatus?: "passed" | "failed";
  latestTime?: string;
  latestReportPath?: string;
}

export interface TestRun {
  id: string;
  menuId: string;
  project: "client" | "APP";
  menuName: string;
  mode: "mock" | "real" | "source-style" | "browser-style";
  status: "passed" | "failed";
  startedAt: string;
  finishedAt?: string;
  reportMarkdown: string;
  sourceReportPath?: string;
  outputExcerpt: string;
  errorMessage: string;
}

export interface StartTestOptions {
  menuId: string;
  mode: TestRun["mode"];
  account?: string;
  token?: string;
  useEnvironmentToken?: boolean;
}

export interface VideoItem {
  id: string;
  title: string;
  project: string;
  path: string;
  folder: string;
  sourceRoot: string;
  fileName: string;
  extension: string;
  sizeBytes: number;
  modifiedAt: string;
  status: "final" | "output" | "render";
  coverPath?: string;
}

export interface VideoDeliverable {
  kind: "video" | "cover" | "script" | "publish";
  label: string;
  path?: string;
  fileName?: string;
  content?: string;
  available: boolean;
}

export interface VideoProjectDetails {
  projectRoot: string;
  deliverables: VideoDeliverable[];
}

export type WorkSessionSource = "estimated" | "manual";

export interface WorkSession {
  id: string;
  date: string;
  startTime: string;
  endTime: string;
  durationMinutes: number;
  project: string;
  workType: string;
  source: WorkSessionSource;
  note: string;
  createdAt: string;
  updatedAt: string;
}

export interface WorkBreakdown {
  name: string;
  minutes: number;
}

export interface DailyWorkMinutes {
  date: string;
  minutes: number;
  estimatedMinutes: number;
  manualMinutes: number;
}

export interface WorkSummary {
  startDate: string;
  endDate: string;
  totalMinutes: number;
  estimatedMinutes: number;
  manualMinutes: number;
  hasManualCorrections: boolean;
  byProject: WorkBreakdown[];
  byType: WorkBreakdown[];
  daily: DailyWorkMinutes[];
}

export interface WorkTimeSettings {
  gapMinutes: number;
}

export function isTauriRuntime() {
  return "__TAURI_INTERNALS__" in window;
}

export async function listTasks(): Promise<WorkTask[]> {
  return invoke<WorkTask[]>("list_tasks");
}

export async function saveTask(task: WorkTask): Promise<void> {
  await invoke("save_task", { task });
}

export async function deleteTask(id: string): Promise<void> {
  await invoke("delete_task", { id });
}

export async function databaseHealth(): Promise<DatabaseHealth> {
  return invoke<DatabaseHealth>("database_health");
}

export async function scanCodexSessions(): Promise<CodexScanSummary> {
  return invoke<CodexScanSummary>("scan_codex_sessions");
}

export async function getCodexQuota(): Promise<CodexQuotaSnapshot> {
  return invoke<CodexQuotaSnapshot>("codex_quota");
}

export async function scanGitRepositories(): Promise<GitScanSummary> {
  return invoke<GitScanSummary>("scan_git_repositories");
}

export async function getGitScanConfiguration(): Promise<GitScanConfiguration> {
  return invoke<GitScanConfiguration>("git_scan_configuration");
}

export async function saveGitScanConfiguration(configuration: GitScanConfiguration): Promise<void> {
  await invoke("save_git_scan_configuration", { configuration });
}

export async function listRepositoryAssets(): Promise<RepositoryAsset[]> {
  return invoke<RepositoryAsset[]>("list_repository_assets");
}

export async function getRepositoryAssetDetails(path: string): Promise<RepositoryAssetDetails> {
  return invoke<RepositoryAssetDetails>("repository_asset_details", { path });
}

export async function saveRepositoryAsset(asset: RepositoryAssetUpdate): Promise<void> {
  await invoke("save_repository_asset", { asset });
}

export async function getTokenSummary(): Promise<TokenSummary> {
  return invoke<TokenSummary>("token_summary");
}

export async function listConversationMetrics(limit = 50): Promise<ConversationMetric[]> {
  return invoke<ConversationMetric[]>("list_conversation_metrics", { limit });
}

export async function setConversationProject(id: string, project?: string): Promise<void> {
  await invoke("set_conversation_project", { id, project });
}

export async function getTokenTrend(days = 14): Promise<TokenTrendPoint[]> {
  return invoke<TokenTrendPoint[]>("token_trend", { days });
}

export async function getProjectTokenMetrics(): Promise<ProjectTokenMetric[]> {
  return invoke<ProjectTokenMetric[]>("project_token_metrics");
}

export async function getModelTokenMetrics(): Promise<ModelTokenMetric[]> {
  return invoke<ModelTokenMetric[]>("model_token_metrics");
}

export async function searchWorkspace(query: string): Promise<WorkspaceSearchResult[]> {
  return invoke<WorkspaceSearchResult[]>("search_workspace", { query });
}

export async function syncTaskSuggestions(): Promise<SuggestionSyncSummary> {
  return invoke<SuggestionSyncSummary>("sync_task_suggestions");
}

export async function listContentIdeas(date?: string, contentType: ContentIdea["contentType"] = "tech"): Promise<ContentIdea[]> {
  return invoke<ContentIdea[]>("list_content_ideas", { date, contentType });
}

export async function generateDailyContent(date?: string, force = false, useAi = true, contentType: ContentIdea["contentType"] = "tech"): Promise<ContentIdea[]> {
  return invoke<ContentIdea[]>("generate_daily_content", { date, force, useAi, contentType });
}

export async function updateContentStatus(id: string, status: ContentIdea["status"]): Promise<void> {
  await invoke("update_content_status", { id, status });
}

export async function listReports(): Promise<ReportRecord[]> {
  return invoke<ReportRecord[]>("list_reports");
}

export async function getReportSources(reportId: string): Promise<ReportSource[]> {
  return invoke<ReportSource[]>("report_sources", { reportId });
}

export async function generateReport(reportType: ReportRecord["reportType"], referenceDate: string): Promise<ReportRecord> {
  return invoke<ReportRecord>("generate_report", { reportType, referenceDate });
}

export async function backfillHistoricalReports(): Promise<HistoricalReportSummary> {
  return invoke<HistoricalReportSummary>("backfill_historical_reports");
}

export async function getDailyActivity(startDate: string, endDate: string): Promise<DailyActivity[]> {
  return invoke<DailyActivity[]>("daily_activity", { startDate, endDate });
}

export async function getHistoryCoverage(): Promise<HistoryCoverage> {
  return invoke<HistoryCoverage>("history_coverage");
}

export async function saveReport(report: ReportRecord): Promise<ReportRecord> {
  return invoke<ReportRecord>("save_report", { report });
}

export async function setReportLocked(id: string, locked: boolean): Promise<void> {
  await invoke("set_report_locked", { id, locked });
}

export async function listKnowledge(): Promise<KnowledgeItem[]> {
  return invoke<KnowledgeItem[]>("list_knowledge");
}

export async function syncKnowledge(): Promise<KnowledgeSyncSummary> {
  return invoke<KnowledgeSyncSummary>("sync_knowledge");
}

export async function saveKnowledge(item: KnowledgeItem): Promise<KnowledgeItem> {
  return invoke<KnowledgeItem>("save_knowledge", { item });
}

export async function deleteKnowledge(id: string): Promise<void> {
  await invoke("delete_knowledge", { id });
}

export async function getAiStatus(): Promise<AiStatus> {
  return invoke<AiStatus>("ai_status");
}

export async function saveDeepSeekKey(key: string): Promise<void> {
  await invoke("save_deepseek_key", { key });
}

export async function clearDeepSeekKey(): Promise<void> {
  await invoke("clear_deepseek_key");
}

export async function testDeepSeek(): Promise<string> {
  return invoke<string>("test_deepseek");
}

export async function refineReportWithAi(id: string): Promise<string> {
  return invoke<string>("refine_report_with_ai", { id });
}

export async function askKnowledge(question: string): Promise<KnowledgeAnswer> {
  return invoke<KnowledgeAnswer>("ask_knowledge", { question });
}

export async function listTestMenus(): Promise<TestMenu[]> {
  return invoke<TestMenu[]>("list_test_menus");
}

export async function listTestRuns(menuId?: string): Promise<TestRun[]> {
  return invoke<TestRun[]>("list_test_runs", { menuId });
}

export async function readTestReport(path: string): Promise<string> {
  return invoke<string>("read_test_report", { path });
}

export async function startTestRun(options: StartTestOptions): Promise<TestRun> {
  return invoke<TestRun>("start_test_run", { options });
}

export async function listLocalVideos(): Promise<VideoItem[]> {
  return invoke<VideoItem[]>("list_local_videos");
}

export async function readVideoCover(path: string): Promise<string> {
  return invoke<string>("read_video_cover", { path });
}

export async function openLocalVideo(path: string): Promise<void> {
  await invoke("open_local_video", { path });
}

export async function revealLocalVideo(path: string): Promise<void> {
  await invoke("reveal_local_video", { path });
}

export async function getVideoProjectDetails(path: string): Promise<VideoProjectDetails> {
  return invoke<VideoProjectDetails>("video_project_details", { path });
}

export async function revealLocalFile(path: string): Promise<void> {
  await invoke("reveal_local_file", { path });
}

export async function listWorkSessions(startDate: string, endDate: string, refresh = true): Promise<WorkSession[]> {
  return invoke<WorkSession[]>("list_work_sessions", { startDate, endDate, refresh });
}

export async function getWorkSummary(startDate: string, endDate: string, refresh = true): Promise<WorkSummary> {
  return invoke<WorkSummary>("work_summary", { startDate, endDate, refresh });
}

export async function saveWorkSession(session: WorkSession): Promise<WorkSession> {
  return invoke<WorkSession>("save_work_session", { session });
}

export async function deleteWorkSession(id: string): Promise<void> {
  await invoke("delete_work_session", { id });
}

export async function getWorkTimeSettings(): Promise<WorkTimeSettings> {
  return invoke<WorkTimeSettings>("work_time_settings");
}

export async function saveWorkTimeSettings(gapMinutes: number): Promise<WorkTimeSettings> {
  return invoke<WorkTimeSettings>("save_work_time_settings", { gapMinutes });
}
