import type { JenkinsPublishRecord } from "../services/backend";

const ACTIVE_STATUSES = new Set<JenkinsPublishRecord["status"]>(["queued", "running"]);
const INCOMPLETE_STAGE_STATUSES = new Set(["IN_PROGRESS", "PAUSED_PENDING_INPUT", "UNKNOWN"]);

export interface JenkinsActiveOperation {
  id: string;
  kind: "jenkins";
  title: string;
  detail: string;
  status: string;
  href: string;
  progressPercent?: number;
  etaText: string;
}

function runningDurationText(record: JenkinsPublishRecord, currentTime: number) {
  const startedAt = Date.parse(record.buildStartedAt || record.startedAt);
  if (!Number.isFinite(startedAt)) return "";
  const seconds = Math.max(0, Math.floor((currentTime - startedAt) / 1_000));
  if (seconds < 60) return `${seconds} 秒`;
  const minutes = Math.floor(seconds / 60);
  return minutes < 60 ? `${minutes} 分钟` : `${Math.floor(minutes / 60)} 小时 ${minutes % 60} 分`;
}

export function buildJenkinsActiveOperations(records: JenkinsPublishRecord[], currentTime = Date.now()): JenkinsActiveOperation[] {
  return records.filter(record => ACTIVE_STATUSES.has(record.status)).map(record => {
    const completedStages = record.stages.filter(stage => !INCOMPLETE_STAGE_STATUSES.has(stage.status)).length;
    const progressPercent = record.stages.length
      ? Math.round((completedStages / record.stages.length) * 100)
      : undefined;
    const reconnecting = record.syncState === "reconnecting";
    const progressDetail = reconnecting
      ? record.errorMessage || "暂时无法连接 Jenkins"
      : record.status === "queued"
        ? record.queueReason || "等待 Jenkins 分配执行节点"
        : record.currentStage
          ? `当前阶段：${record.currentStage}`
          : "Jenkins 正在执行发布任务";
    const etaText = [
      record.buildNumber ? `#${record.buildNumber}` : "",
      runningDurationText(record, currentTime),
      record.stages.length ? `${completedStages} / ${record.stages.length} 阶段` : "",
    ].filter(Boolean).join(" · ");

    return {
      id: `jenkins:${record.id}`,
      kind: "jenkins" as const,
      title: record.jobName || record.jobFullName,
      detail: `${record.branch} · ${progressDetail}`,
      status: reconnecting ? "连接中断，正在重试" : record.status === "queued" ? "排队中" : "发布中",
      href: `/deployments?run=${encodeURIComponent(record.id)}`,
      progressPercent,
      etaText,
    };
  });
}
