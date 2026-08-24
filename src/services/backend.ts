import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type { WorkTask } from "../types/workbench";

let activeBackendRequests = 0;

function emitBackendLoading(command: string) {
  if (typeof window === "undefined") return;
  window.dispatchEvent(new CustomEvent("workbench-backend-loading", {
    detail: { active: activeBackendRequests, command },
  }));
}

/** 统一记录本地命令的请求状态，供页面壳层展示加载反馈。 */
async function invoke<T = void>(command: string, args?: Record<string, unknown>): Promise<T> {
  activeBackendRequests += 1;
  emitBackendLoading(command);
  try {
    return await tauriInvoke<T>(command, args);
  } finally {
    activeBackendRequests = Math.max(0, activeBackendRequests - 1);
    emitBackendLoading(command);
  }
}

export interface DatabaseHealth {
  path: string;
  schemaVersion: number;
}

export interface BackupEntry {
  path: string;
  fileName: string;
  kind: "daily" | "manual" | "export" | "pre-restore" | "migration";
  createdAt: string;
  sizeBytes: number;
}

export interface BackupStatus {
  databasePath: string;
  backupDirectory: string;
  backups: BackupEntry[];
}

export interface UpdateStatus {
  currentVersion: string;
  latestVersion: string;
  updateAvailable: boolean;
  publishedAt: string;
  releaseUrl: string;
  installerUrl: string;
  portableUrl: string;
  checkedAt: string;
  message: string;
}

export interface QuickCapture {
  id: string;
  kind: "note" | "idea" | "url";
  content: string;
  sourceUrl: string;
  status: "inbox" | "archived";
  createdAt: string;
  updatedAt: string;
}

export interface DailyCheckin {
  date: string;
  energy?: number;
  mood: string;
  exerciseMinutes: number;
  note: string;
  createdAt: string;
  updatedAt: string;
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
  excludedNames: string[];
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
  sourceFile?: string;
  sourceModifiedAt?: string;
  freshness: "fresh" | "recent" | "stale" | "";
  selectionReason: string;
}

export interface CodexCliStatus {
  installed: boolean;
  authenticated: boolean;
  version: string;
  executablePath: string;
  message: string;
}

export interface WorkbenchNotification {
  id: string;
  kind: "codex_complete" | "codex_task" | "tapd_item";
  title: string;
  body: string;
  output: string;
  sourceId?: string;
  route: string;
  isRead: boolean;
  createdAt: string;
  readAt?: string;
  reviewStatus: "pending" | "accepted" | "follow_up";
  reviewNote: string;
  reviewedAt?: string;
}

export type InboxWorkflowStatus = "needs_decision" | "in_progress" | "done" | "archived";

export interface InboxItem {
  id: string;
  sourceType: "codex" | "tapd" | "tapd_job" | "task_suggestion" | "test" | "repository" | "video";
  sourceId: string;
  project: string;
  title: string;
  summary: string;
  detail: string;
  route: string;
  priority: "high" | "normal" | "low";
  workflowStatus: InboxWorkflowStatus;
  sourceStatus: string;
  createdAt: string;
  updatedAt: string;
}

export interface ProjectProfile {
  id: string;
  displayName: string;
  repositoryPath: string;
  tapdWorkspaceId: string;
  aliases: string[];
  category: string;
  createdAt: string;
  updatedAt: string;
}

export interface ProjectProfileUpdate {
  id: string;
  displayName: string;
  repositoryPath: string;
  tapdWorkspaceId: string;
  aliases: string[];
  category: string;
}

export interface NotificationSyncSummary {
  filesScanned: number;
  notificationsCreated: number;
}

export interface EmailNotificationStatus {
  configured: boolean;
  enabled: boolean;
  state: "unconfigured" | "unverified" | "disabled" | "ready" | "error";
  maskedEmail: string;
  lastError: string;
  retryingCount: number;
  failedCount: number;
}

export interface VipStatus {
  active: boolean;
}

export interface GitScanSummary {
  repositoriesFound: number;
  commitsImported: number;
  snapshotsCreated: number;
  errors: number;
  errorDetails: string[];
}

export interface GitScanStatus {
  lastScannedAt: string;
  errors: string[];
}

export interface RepositoryAsset {
  path: string;
  name: string;
  isPinned: boolean;
  isHidden: boolean;
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
  changedFileCount: number;
  aheadCount: number;
  behindCount: number;
  inferenceStatus: string;
  manuallyConfirmed: boolean;
  lastScannedAt: string;
  updatedAt: string;
  healthLevel: string;
  healthSummary: string;
  commitCount: number;
  conversationCount: number;
  lastActivityAt: string;
  runtimeStatus: "" | "starting" | "running" | "failed" | "stopped";
  runtimeLocalUrl: string;
  runtimeError: string;
  runtimeStartedAt: string;
  runtimeLogPath: string;
  runtimeLogExcerpt: string;
  pendingLevel: "none" | "low" | "medium" | "high";
  pendingSummary: string;
  nextAction: string;
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

export interface ProjectLaunchResult {
  projectPath: string;
  projectName: string;
  command: string;
  processId: number;
  managed: boolean;
  message: string;
  status: string;
  startedAt: string;
  localUrl: string;
  logPath: string;
}

export interface RunningProjectProcess {
  projectPath: string;
  projectName: string;
  command: string;
  processId: number;
  status: "starting" | "running" | "failed" | "stopped";
  startedAt: string;
  localUrl: string;
  logPath: string;
  logExcerpt: string;
  errorMessage: string;
}

export interface RepositoryAssociation {
  id: string;
  kind: "codex" | "test" | "work" | "tapd" | "report" | "deployment" | "docs" | "build" | "remote" | "runtime";
  title: string;
  subtitle: string;
  status: string;
  updatedAt: string;
  route: string;
}

export interface RepositoryAssetDetails {
  conversations: Array<{ id: string; title: string; updatedAt: string; archived: boolean }>;
  commits: Array<{ hash: string; subject: string; committedAt: string }>;
  commitPlan?: CommitPlan;
  associations: RepositoryAssociation[];
  nextAction: string;
}

export interface TapdStatus {
  configured: boolean;
  source: string;
  authMode: "token" | "basic";
  workspaceId: string;
  workspaceName: string;
  owner: string;
  lastSyncedAt?: string;
  itemCount: number;
  warnings: string[];
  autoFixEnabled: boolean;
  autoFixRepositoryPath: string;
  automationPaused: boolean;
  projects: TapdProjectConfig[];
}

export interface TapdProjectConfig {
  workspaceId: string;
  workspaceName: string;
  owner: string;
  enabled: boolean;
  sortOrder: number;
  repositoryPath: string;
  autoEnabled: boolean;
  autoExecute: boolean;
  triggerStatuses: string[];
  completionStatus: string;
  lastSyncedAt?: string;
  lastError: string;
  itemCount: number;
}

export interface TapdProjectInput {
  workspaceId: string;
  workspaceName: string;
  owner: string;
  enabled: boolean;
  sortOrder: number;
}

export interface TapdProjectAutomationInput {
  workspaceId: string;
  repositoryPath: string;
  autoEnabled: boolean;
  autoExecute: boolean;
  triggerStatuses: string[];
  completionStatus: string;
}

export interface GitCredentialStatus {
  configured: boolean;
  username: string;
  source: string;
}

export interface GitChangedFile {
  path: string;
  indexStatus: string;
  worktreeStatus: string;
  label: string;
}

export interface GitRepositoryStatus {
  repositoryPath: string;
  currentBranch: string;
  branches: string[];
  remoteUrl: string;
  upstream: string;
  ahead: number;
  behind: number;
  userName: string;
  userEmail: string;
  hasUncommittedChanges: boolean;
  changedFiles: GitChangedFile[];
  credential: GitCredentialStatus;
}

export interface GitOperationResult {
  message: string;
  output: string;
  commitHash: string;
}

export interface TapdWorkItem {
  itemKey: string;
  id: string;
  workspaceId: string;
  itemType: "bug" | "task" | "story";
  title: string;
  description: string;
  status: string;
  statusLabel: string;
  priority: string;
  owner: string;
  creator: string;
  iterationId: string;
  beginDate: string;
  dueDate: string;
  createdAt: string;
  modifiedAt: string;
  sourceUrl: string;
  syncedAt: string;
}

export interface TapdSyncSummary {
  projectsSynced: number;
  bugs: number;
  tasks: number;
  stories: number;
  total: number;
  notificationsCreated: number;
  warnings: string[];
  syncedAt: string;
  autoJobsStarted: number;
  autoJobsQueued: number;
  autoJobsSkipped: number;
}

export interface TapdCodexJob {
  id: string;
  itemKey: string;
  itemId: string;
  workspaceId: string;
  repositoryPath: string;
  status: "queued" | "running" | "completed" | "failed";
  threadId?: string;
  output: string;
  errorMessage: string;
  baselineHead: string;
  baselineWorktree: string;
  resultHead: string;
  changedFiles: string[];
  testSummary: string;
  reviewStatus: "pending" | "accepted" | "changes_requested";
  reviewNote: string;
  reviewedAt?: string;
  triggerSource: "manual" | "auto";
  sourceModifiedAt: string;
  triggerReason: string;
  executionMode: "automatic" | "manual";
  executionBlockReason: string;
  startedAt?: string;
  completedAt?: string;
  testRequired: boolean;
  processReportPath: string;
  createdAt: string;
  updatedAt: string;
}

export interface TapdAutoFixSettings {
  enabled: boolean;
  repositoryPath: string;
}

export interface CommitPlanGroup {
  id: string; title: string; commitMessage: string; files: string[]; riskNotes: string; verificationNotes: string; status: string;
}

export interface CommitPlan {
  id: string; repositoryPath: string; status: string; riskLevel: string; summary: string; createdAt: string; groups: CommitPlanGroup[];
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
  quickCaptureCount: number;
  completedVideoCount: number;
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
  kind: "Codex 对话" | "Git 提交" | "任务" | "测试" | "TAPD 缺陷";
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

export type TestMode = "mock" | "real" | "source-style" | "browser-style";
export type TestRunStatus = "queued" | "running" | "passed" | "failed" | "blocked" | "error" | "cancelled";

export interface TestProject {
  path: string;
  name: string;
  projectKind: "vue" | "uni-app" | "web" | "unknown";
  caseCount: number;
  pageCount: number;
  capabilities: TestCapabilities;
  warnings: string[];
}

export interface TestScenario {
  id: string;
  title: string;
  description: string;
  mode: TestMode;
  defaultSelected: boolean;
}

export interface TestArtifact {
  name: string;
  path: string;
  contentType: string;
  kind: "screenshot" | "trace" | "log" | "attachment";
}

export interface TestScenarioResult {
  id: string;
  title: string;
  status: "passed" | "failed" | "skipped" | "blocked";
  durationMs: number;
  purpose: string;
  steps: string[];
  checks: string[];
  errorMessage: string;
  artifacts: TestArtifact[];
}

export interface TestPreflight {
  ready: boolean;
  status: "ready" | "blocked";
  checks: Array<{ name: string; passed: boolean; detail: string }>;
  warnings: string[];
}

export interface TestMenu {
  id: string;
  project: string;
  projectPath: string;
  projectKind: TestProject["projectKind"];
  name: string;
  route: string;
  sourcePath: string;
  caseId?: string;
  hasCaseFile: boolean;
  caseFilePath?: string;
  canCreateCaseFile: boolean;
  capabilities: TestCapabilities;
  tested: boolean;
  latestStatus?: TestRunStatus;
  latestTime?: string;
  latestReportPath?: string;
}

export interface TestRun {
  id: string;
  menuId: string;
  project: string;
  projectPath: string;
  menuName: string;
  mode: TestMode;
  status: TestRunStatus;
  startedAt: string;
  finishedAt?: string;
  reportMarkdown: string;
  sourceReportPath?: string;
  outputExcerpt: string;
  errorMessage: string;
  selectedScenarios: string[];
  scenarioResults: TestScenarioResult[];
  artifacts: TestArtifact[];
  totalCount: number;
  passedCount: number;
  failedCount: number;
  skippedCount: number;
  durationMs: number;
  exitCode?: number;
  environmentSummary: string;
  cleanupStatus: "not-applicable" | "completed" | "failed" | "unknown";
}

export interface StartTestOptions {
  projectPath: string;
  menuId: string;
  mode: TestMode;
  selectedScenarios: string[];
  createCaseFile?: boolean;
  confirmedRealWrite?: boolean;
  account?: string;
  token?: string;
  useEnvironmentToken?: boolean;
}

export interface ApiContract {
  id: string;
  featureId: string;
  platform: "PC" | "APP";
  method: string;
  url: string;
  sourceFile: string;
  verificationLevel: "static" | "api" | "browser";
}

export interface RegressionEvidence {
  platform: "PC" | "APP";
  verificationType: "static" | "api" | "browser";
  status: "passed" | "failed" | "unverified";
  resultSummary: string;
  sourcePath: string;
  verifiedAt?: string;
}

export interface FeatureParity {
  id: string;
  domain: string;
  featureName: string;
  pcPage: string;
  appPage: string;
  parityStatus: "pending" | "static-aligned" | "confirmed" | "different" | "pc-only" | "app-only";
  evidence: string[];
  intentionalDifference: boolean;
  manuallyConfirmed: boolean;
  updatedAt: string;
  contracts: ApiContract[];
  regression: RegressionEvidence[];
}

export interface ParitySyncSummary {
  featureCount: number;
  pcFeatureCount: number;
  appFeatureCount: number;
  matchedCount: number;
  pcOnlyCount: number;
  appOnlyCount: number;
  contractCount: number;
  regressionCount: number;
  alignedCount: number;
  pendingCount: number;
  sourceMessage: string;
}

export interface ToolchainInstallation {
  id: string;
  toolName: string;
  version: string;
  executablePath: string;
  source: string;
  pathPriority: number;
  scannedAt: string;
}

export interface ToolchainConflict {
  id: string;
  toolName: string;
  conflictType: "multiple-paths" | "version-mismatch";
  summary: string;
  recommendedAction: string;
  status: "unconfirmed" | "confirmed" | "ignored";
  detectedAt: string;
}

export interface ToolchainInventory {
  installations: ToolchainInstallation[];
  conflicts: ToolchainConflict[];
}

export interface AuditCheck {
  checkType: string;
  target: string;
  status: "passed" | "attention" | "failed";
  summary: string;
  detailsJson: string;
  checkedAt?: string;
}

export interface WeeklyAudit {
  id: string;
  weekStart: string;
  status: "passed" | "attention" | "failed" | "running";
  scheduledAt: string;
  startedAt?: string;
  finishedAt?: string;
  summary: string;
  catchUpRun: boolean;
  checks: AuditCheck[];
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
  collection: "human-weakness" | "tech" | "reasoning";
}

export interface VideoJobDeliverable {
  kind: "script" | "video" | "cover" | "publish";
  path: string;
  status: "ready" | "missing";
  qualitySummary: string;
  checkedAt?: string;
}

export interface VideoJob {
  id: string;
  title: string;
  videoType: "human-weakness" | "tech" | "reasoning";
  status: "queued" | "running" | "finalizing" | "complete" | "needs-attention" | "failed";
  currentStage: "selection" | "codex" | "script" | "assets" | "voice" | "composition" | "quality" | "render" | "finalizing" | "failed" | "cover" | "publish" | "delivery";
  progressPercent: number;
  progressMessage: string;
  lastProgressAt?: string;
  projectRoot: string;
  failureReason: string;
  manuallyConfirmedType: boolean;
  contentIdeaId?: string;
  skillName: string;
  codexThreadId?: string;
  codexOutput: string;
  cliLogPath: string;
  startedAt?: string;
  completedAt?: string;
  createdAt: string;
  updatedAt: string;
  deliverables: VideoJobDeliverable[];
}

export interface VideoPipelineSummary {
  jobCount: number;
  completeCount: number;
  needsAttentionCount: number;
  techSamples: number;
  reasoningSamples: number;
  humanWeaknessSamples: number;
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

export interface VideoPublishRecord {
  id: string;
  videoJobId: string;
  title: string;
  videoType: VideoJob["videoType"];
  platform: string;
  status: "ready" | "published";
  publishUrl: string;
  publishedAt?: string;
  views: number;
  likes: number;
  comments: number;
  favorites: number;
  notes: string;
  updatedAt: string;
}

export type SaveVideoPublishRecord = Omit<VideoPublishRecord, "id" | "title" | "videoType" | "updatedAt">;

export interface KnowledgeVersion {
  id: string;
  knowledgeId: string;
  versionNumber: number;
  title: string;
  content: string;
  tags: string;
  changeSource: "manual_edit" | "auto_sync";
  createdAt: string;
}

export interface KnowledgeCodexJob {
  id: string;
  knowledgeId: string;
  repositoryPath: string;
  instruction: string;
  status: "running" | "completed" | "failed";
  threadId?: string;
  output: string;
  errorMessage: string;
  createdAt: string;
  updatedAt: string;
}

export interface TestRecommendation {
  menuId: string;
  project: string;
  projectPath: string;
  menuName: string;
  changedFiles: string[];
  reason: string;
  recommendedMode: TestRun["mode"];
}

export async function getBackupStatus(): Promise<BackupStatus> {
  return invoke<BackupStatus>("backup_status");
}

export async function createDatabaseBackup(): Promise<BackupEntry> {
  return invoke<BackupEntry>("create_database_backup");
}

export async function exportDatabaseBackup(): Promise<BackupEntry> {
  return invoke<BackupEntry>("export_database_backup");
}

export async function restoreDatabaseBackup(path: string): Promise<BackupStatus> {
  return invoke<BackupStatus>("restore_database_backup", { path });
}

export async function checkForUpdates(): Promise<UpdateStatus> {
  return invoke<UpdateStatus>("check_for_updates");
}

export async function listQuickCaptures(includeArchived = false): Promise<QuickCapture[]> {
  return invoke<QuickCapture[]>("list_quick_captures", { includeArchived });
}

export async function saveQuickCapture(input: Pick<QuickCapture,"kind"|"content"|"sourceUrl">): Promise<QuickCapture> {
  return invoke<QuickCapture>("save_quick_capture", { input });
}

export async function archiveQuickCapture(id: string): Promise<void> {
  await invoke("archive_quick_capture", { id });
}

export interface TapdAutomationPreviewItem {
  itemKey: string;
  itemId: string;
  title: string;
  statusLabel: string;
  priority: string;
  dueDate: string;
  triggerReason: string;
}

export interface TapdAutomationPreview {
  workspaceId: string;
  totalItems: number;
  matchedCount: number;
  pendingCount: number;
  items: TapdAutomationPreviewItem[];
}

export async function deleteQuickCapture(id: string): Promise<void> {
  await invoke("delete_quick_capture", { id });
}

export async function getDailyCheckin(date: string): Promise<DailyCheckin | null> {
  return invoke<DailyCheckin | null>("get_daily_checkin", { date });
}

export async function saveDailyCheckin(checkin: DailyCheckin): Promise<DailyCheckin> {
  return invoke<DailyCheckin>("save_daily_checkin", { checkin });
}

export async function scanCodexSessions(): Promise<CodexScanSummary> {
  return invoke<CodexScanSummary>("scan_codex_sessions");
}

export async function getCodexQuota(): Promise<CodexQuotaSnapshot> {
  return invoke<CodexQuotaSnapshot>("codex_quota");
}

export async function getCodexCliStatus(): Promise<CodexCliStatus> {
  return invoke<CodexCliStatus>("codex_cli_status");
}

export async function syncCodexNotifications(): Promise<NotificationSyncSummary> {
  return invoke<NotificationSyncSummary>("sync_codex_notifications");
}

export async function listNotifications(limit = 30): Promise<WorkbenchNotification[]> {
  return invoke<WorkbenchNotification[]>("list_notifications", { limit });
}

export async function markNotificationRead(id: string): Promise<void> {
  await invoke("mark_notification_read", { id });
}

export async function markAllNotificationsRead(): Promise<void> {
  await invoke("mark_all_notifications_read");
}

export async function reviewNotification(id: string, decision: "accepted" | "follow_up", note = ""): Promise<void> {
  await invoke("review_notification", { id, decision, note });
}

export async function listInboxItems(status?: InboxWorkflowStatus, limit = 200): Promise<InboxItem[]> {
  return invoke<InboxItem[]>("list_inbox_items", { status: status || null, limit });
}

export async function updateInboxStatus(id: string, status: InboxWorkflowStatus): Promise<void> {
  await invoke("update_inbox_status", { id, status });
}

export async function createTaskFromInbox(id: string): Promise<string> {
  return invoke<string>("create_task_from_inbox", { id });
}

export async function listProjectProfiles(): Promise<ProjectProfile[]> {
  return invoke<ProjectProfile[]>("list_project_profiles");
}

export async function saveProjectProfile(profile: ProjectProfileUpdate): Promise<ProjectProfile> {
  return invoke<ProjectProfile>("save_project_profile", { profile });
}

export async function getEmailNotificationStatus(): Promise<EmailNotificationStatus> {
  return invoke<EmailNotificationStatus>("email_notification_status");
}

export async function saveQqEmailConfig(email: string, authCode: string): Promise<void> {
  await invoke("save_qq_email_config", { email, authCode });
}

export async function deleteQqEmailConfig(): Promise<void> {
  await invoke("delete_qq_email_config");
}

export async function testQqEmail(): Promise<string> {
  return invoke<string>("test_qq_email");
}

export async function setCodexEmailEnabled(enabled: boolean): Promise<EmailNotificationStatus> {
  return invoke<EmailNotificationStatus>("set_codex_email_enabled", { enabled });
}

export async function retryFailedEmails(): Promise<EmailNotificationStatus> {
  return invoke<EmailNotificationStatus>("retry_failed_emails");
}

export async function getVipStatus(): Promise<VipStatus> {
  return invoke<VipStatus>("vip_status");
}

export async function activateVip(code: string): Promise<VipStatus> {
  return invoke<VipStatus>("activate_vip", { code });
}

export async function deactivateVip(): Promise<VipStatus> {
  return invoke<VipStatus>("deactivate_vip");
}

export async function scanGitRepositories(): Promise<GitScanSummary> {
  return invoke<GitScanSummary>("scan_git_repositories");
}

export async function getGitScanConfiguration(): Promise<GitScanConfiguration> {
  return invoke<GitScanConfiguration>("git_scan_configuration");
}

export async function getGitScanStatus(): Promise<GitScanStatus> {
  return invoke<GitScanStatus>("git_scan_status");
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

export async function generateCommitPlan(path: string): Promise<CommitPlan> {
  return invoke<CommitPlan>("generate_commit_plan", { path });
}

export async function setRepositoryPinned(path: string, pinned: boolean): Promise<void> {
  await invoke("set_repository_pinned", { path, pinned });
}

export async function setRepositoryHidden(path: string, hidden: boolean): Promise<void> {
  await invoke("set_repository_hidden", { path, hidden });
}

export async function setRepositoryCategory(path: string, category: string): Promise<void> {
  await invoke("set_repository_category", { path, category });
}

export async function startRepositoryProject(path: string): Promise<ProjectLaunchResult> {
  return invoke<ProjectLaunchResult>("start_repository_project", { path });
}

export async function stopRepositoryProject(path: string): Promise<ProjectLaunchResult> {
  return invoke<ProjectLaunchResult>("stop_repository_project", { path });
}

export async function listRunningRepositoryProjects(): Promise<RunningProjectProcess[]> {
  return invoke<RunningProjectProcess[]>("list_running_repository_projects");
}

export async function openRepositoryRuntimeUrl(url: string): Promise<void> {
  await invoke("open_repository_runtime_url", { url });
}

export async function getGitCredentialStatus(): Promise<GitCredentialStatus> {
  return invoke<GitCredentialStatus>("git_credential_status");
}

export async function saveGitDefaultCredential(username: string, password: string): Promise<void> {
  await invoke("save_git_default_credential", { username, password });
}

export async function clearGitDefaultCredential(): Promise<void> {
  await invoke("clear_git_default_credential");
}

export async function getGitRepositoryStatus(path: string): Promise<GitRepositoryStatus> {
  return invoke<GitRepositoryStatus>("git_repository_status", { path });
}

export async function fetchGitRepository(path: string): Promise<GitOperationResult> {
  return invoke<GitOperationResult>("git_fetch_repository", { path });
}

export async function pullGitRepository(path: string): Promise<GitOperationResult> {
  return invoke<GitOperationResult>("git_pull_repository", { path });
}

export async function stageGitRepositoryChanges(path: string): Promise<GitOperationResult> {
  return invoke<GitOperationResult>("git_stage_repository_changes", { path });
}

export async function pushGitRepository(path: string): Promise<GitOperationResult> {
  return invoke<GitOperationResult>("git_push_repository", { path });
}

export async function switchGitRepositoryBranch(path: string, branch: string): Promise<GitOperationResult> {
  return invoke<GitOperationResult>("git_switch_repository_branch", { path, branch });
}

export async function mergeGitRepositoryBranch(path: string, branch: string): Promise<GitOperationResult> {
  return invoke<GitOperationResult>("git_merge_repository_branch", { path, branch });
}

export async function revertGitRepositoryCommit(path: string, commitHash: string): Promise<GitOperationResult> {
  return invoke<GitOperationResult>("git_revert_repository_commit", { path, commitHash });
}

export async function commitGitPlanGroup(path: string, groupId: string, commitMessage: string): Promise<GitOperationResult> {
  return invoke<GitOperationResult>("execute_commit_plan_group", { path, groupId, commitMessage });
}

export async function getTapdStatus(): Promise<TapdStatus> {
  return invoke<TapdStatus>("tapd_status");
}

export async function listTapdProjects(): Promise<TapdProjectConfig[]> {
  return invoke<TapdProjectConfig[]>("list_tapd_projects");
}

export async function saveTapdProject(project: TapdProjectInput): Promise<TapdProjectConfig> {
  return invoke<TapdProjectConfig>("save_tapd_project", { project });
}

export async function removeTapdProject(workspaceId: string): Promise<void> {
  await invoke("remove_tapd_project", { workspaceId });
}

export async function saveTapdProjectAutomation(settings: TapdProjectAutomationInput): Promise<TapdProjectConfig> {
  return invoke<TapdProjectConfig>("save_tapd_project_automation", { settings });
}

export async function previewTapdProjectAutomation(settings: TapdProjectAutomationInput): Promise<TapdAutomationPreview> {
  return invoke<TapdAutomationPreview>("preview_tapd_project_automation", { settings });
}

export async function setTapdAutomationPaused(paused: boolean): Promise<boolean> {
  return invoke<boolean>("set_tapd_automation_paused", { paused });
}

export async function saveTapdAutoFixSettings(enabled: boolean, repositoryPath: string): Promise<TapdAutoFixSettings> {
  return invoke<TapdAutoFixSettings>("save_tapd_auto_fix_settings", { enabled, repositoryPath });
}

export async function saveTapdCredentials(authMode: "token" | "basic", apiUser: string, apiPassword: string, accessToken: string, owner: string): Promise<void> {
  await invoke("save_tapd_credentials", { authMode, apiUser, apiPassword, accessToken, owner });
}

export async function clearTapdCredentials(): Promise<void> {
  await invoke("clear_tapd_credentials");
}

export async function testTapdConnection(): Promise<string> {
  return invoke<string>("test_tapd_connection");
}

export async function syncTapdItems(): Promise<TapdSyncSummary> {
  return invoke<TapdSyncSummary>("sync_tapd_items");
}

export async function listTapdItems(workspaceId?: string): Promise<TapdWorkItem[]> {
  return invoke<TapdWorkItem[]>("list_tapd_items", { workspaceId: workspaceId || null });
}

export async function listTapdCodexJobs(): Promise<TapdCodexJob[]> {
  return invoke<TapdCodexJob[]>("list_tapd_codex_jobs");
}

export async function executeTapdCodexJob(id: string): Promise<TapdCodexJob> {
  return invoke<TapdCodexJob>("execute_tapd_codex_job", { id });
}

export async function readTapdProcessReport(id: string): Promise<string> {
  return invoke<string>("read_tapd_process_report", { id });
}

export async function startTapdCodexJob(itemKey: string, repositoryPath: string, additionalNote = ""): Promise<TapdCodexJob> {
  return invoke<TapdCodexJob>("start_tapd_codex_job", { itemKey, repositoryPath, additionalNote });
}

export async function continueTapdCodexJob(id: string, note: string): Promise<TapdCodexJob> {
  return invoke<TapdCodexJob>("continue_tapd_codex_job", { id, note });
}

export async function runTapdCodexJobTests(id: string): Promise<TapdCodexJob> {
  return invoke<TapdCodexJob>("run_tapd_codex_job_tests", { id });
}

export async function reviewTapdCodexJob(id: string, decision: "accepted" | "changes_requested", note = "", allowUntested = false): Promise<TapdCodexJob> {
  return invoke<TapdCodexJob>("review_tapd_codex_job", { review: { id, decision, note, allowUntested } });
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

export async function listKnowledgeVersions(knowledgeId: string): Promise<KnowledgeVersion[]> {
  return invoke<KnowledgeVersion[]>("list_knowledge_versions", { knowledgeId });
}

export async function listKnowledgeCodexJobs(knowledgeId?: string): Promise<KnowledgeCodexJob[]> {
  return invoke<KnowledgeCodexJob[]>("list_knowledge_codex_jobs", { knowledgeId });
}

export async function startKnowledgeCodexJob(knowledgeId: string, repositoryPath: string, instruction = ""): Promise<KnowledgeCodexJob> {
  return invoke<KnowledgeCodexJob>("start_knowledge_codex_job", { knowledgeId, repositoryPath, instruction });
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

export async function listTestProjects(): Promise<TestProject[]> {
  return invoke<TestProject[]>("list_test_projects");
}

export async function listTestMenus(projectPath?: string): Promise<TestMenu[]> {
  return invoke<TestMenu[]>("list_test_menus", { projectPath: projectPath || null });
}

export async function recommendTestsFromGit(projectPath?: string): Promise<TestRecommendation[]> {
  return invoke<TestRecommendation[]>("recommend_tests_from_git", { projectPath: projectPath || null });
}

export async function listTestRuns(menuId?: string, projectPath?: string): Promise<TestRun[]> {
  return invoke<TestRun[]>("list_test_runs", { menuId, projectPath: projectPath || null });
}

export async function readTestReport(path: string): Promise<string> {
  return invoke<string>("read_test_report", { path });
}

export async function startTestRun(options: StartTestOptions): Promise<TestRun> {
  return invoke<TestRun>("start_test_run", { options });
}

export async function getTestRun(runId: string): Promise<TestRun> {
  return invoke<TestRun>("get_test_run", { runId });
}

export async function cancelTestRun(runId: string): Promise<TestRun> {
  return invoke<TestRun>("cancel_test_run", { runId });
}

export async function listTestScenarios(projectPath: string, menuId: string, mode: TestMode): Promise<TestScenario[]> {
  return invoke<TestScenario[]>("list_test_scenarios", { projectPath, menuId, mode });
}

export async function preflightTest(options: StartTestOptions): Promise<TestPreflight> {
  return invoke<TestPreflight>("preflight_test", { options });
}

export async function readTestArtifact(runId: string, path: string): Promise<string> {
  return invoke<string>("read_test_artifact", { runId, path });
}

export async function exportTestReportPdf(runId: string): Promise<string> {
  return invoke<string>("export_test_report_pdf", { runId });
}

export async function exportTestReportMarkdown(runId: string): Promise<string> {
  return invoke<string>("export_test_report_markdown", { runId });
}

export async function getExistingTestReportPdf(runId: string): Promise<string | null> {
  return invoke<string | null>("get_existing_test_report_pdf", { runId });
}

export async function openTestReportPdf(path: string): Promise<void> {
  await invoke("open_test_report_pdf", { path });
}

export async function syncFeatureParity(): Promise<ParitySyncSummary> {
  return invoke<ParitySyncSummary>("sync_feature_parity");
}

export async function listFeatureParity(): Promise<FeatureParity[]> {
  return invoke<FeatureParity[]>("list_feature_parity");
}

export async function saveFeatureParityReview(review: Pick<FeatureParity, "id" | "parityStatus" | "intentionalDifference" | "manuallyConfirmed">): Promise<void> {
  await invoke("save_feature_parity_review", { review });
}

export async function scanToolchains(): Promise<ToolchainInventory> {
  return invoke<ToolchainInventory>("scan_toolchains");
}

export async function listToolchains(): Promise<ToolchainInventory> {
  return invoke<ToolchainInventory>("list_toolchains");
}

export async function ensureWeeklyAudit(): Promise<WeeklyAudit | null> {
  return invoke<WeeklyAudit | null>("ensure_weekly_audit");
}

export async function runWeeklyAudit(): Promise<WeeklyAudit> {
  return invoke<WeeklyAudit>("run_weekly_audit");
}

export async function listWeeklyAudits(): Promise<WeeklyAudit[]> {
  return invoke<WeeklyAudit[]>("list_weekly_audits");
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

export async function syncVideoPipeline(): Promise<VideoPipelineSummary> {
  return invoke<VideoPipelineSummary>("sync_video_pipeline");
}

export async function listVideoJobs(): Promise<VideoJob[]> {
  return invoke<VideoJob[]>("list_video_jobs");
}

export async function saveVideoJobType(id: string, videoType: VideoJob["videoType"]): Promise<void> {
  await invoke("save_video_job_type", { selection: { id, videoType } });
}

export async function listVideoPublishRecords(): Promise<VideoPublishRecord[]> {
  return invoke<VideoPublishRecord[]>("list_video_publish_records");
}

export async function saveVideoPublishRecord(record: SaveVideoPublishRecord): Promise<void> {
  await invoke("save_video_publish_record", { record });
}

export async function getContentVideoJob(ideaId: string): Promise<VideoJob | null> {
  return invoke<VideoJob | null>("content_video_job", { ideaId });
}

export async function startContentVideoJob(ideaId: string): Promise<VideoJob> {
  return invoke<VideoJob>("start_content_video_job", { ideaId });
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
