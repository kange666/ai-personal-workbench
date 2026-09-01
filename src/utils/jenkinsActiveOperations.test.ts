import { describe, expect, it } from "vitest";
import type { JenkinsPublishRecord } from "../services/backend";
import { buildJenkinsActiveOperations } from "./jenkinsActiveOperations";

function record(overrides: Partial<JenkinsPublishRecord> = {}): JenkinsPublishRecord {
  return {
    id: "publish-1",
    jobName: "门户前端",
    jobFullName: "业务系统/门户前端",
    jobUrl: "https://jenkins/job/business/job/web/",
    branchParameter: "BRANCH",
    branch: "main",
    queueUrl: "https://jenkins/queue/item/1/",
    buildUrl: "",
    status: "queued",
    syncState: "synced",
    queueReason: "等待可用节点",
    currentStage: "",
    stages: [],
    startedAt: "2026-09-01T08:00:00Z",
    updatedAt: "2026-09-01T08:00:00Z",
    result: "",
    errorMessage: "",
    ...overrides,
  };
}

describe("buildJenkinsActiveOperations", () => {
  it("只保留排队和发布中的记录，完成后从正在执行中移除", () => {
    const operations = buildJenkinsActiveOperations([
      record(),
      record({ id: "publish-2", status: "success" }),
      record({ id: "publish-3", status: "failed" }),
      record({ id: "publish-4", status: "aborted" }),
    ]);

    expect(operations).toHaveLength(1);
    expect(operations[0]).toMatchObject({
      id: "jenkins:publish-1",
      title: "门户前端",
      status: "排队中",
      detail: "main · 等待可用节点",
    });
  });

  it("有阶段数据时显示真实阶段进度，连接中断时显示重试状态", () => {
    const [operation] = buildJenkinsActiveOperations([record({
      status: "running",
      syncState: "reconnecting",
      buildNumber: 42,
      buildStartedAt: "2026-09-01T08:00:00Z",
      errorMessage: "Jenkins 请求超时",
      stages: [
        { id: "1", name: "构建", status: "SUCCESS", durationMs: 1000 },
        { id: "2", name: "发布", status: "IN_PROGRESS", durationMs: 500 },
      ],
    })], Date.parse("2026-09-01T08:02:00Z"));

    expect(operation).toMatchObject({
      status: "连接中断，正在重试",
      detail: "main · Jenkins 请求超时",
      progressPercent: 50,
      etaText: "#42 · 2 分钟 · 1 / 2 阶段",
      href: "/deployments?run=publish-1",
    });
  });

  it("没有阶段接口时仍显示构建编号和持续时间，但不生成百分比", () => {
    const [operation] = buildJenkinsActiveOperations([record({
      status: "running",
      buildNumber: 128,
      buildStartedAt: "2026-09-01T08:00:00Z",
      stages: [],
    })], Date.parse("2026-09-01T08:00:35Z"));

    expect(operation.progressPercent).toBeUndefined();
    expect(operation.etaText).toBe("#128 · 35 秒");
  });
});
