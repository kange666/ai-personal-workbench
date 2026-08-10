function excerpt(value: string, limit: number) {
  const characters = [...value.trim()];
  return characters.length > limit ? `${characters.slice(0, limit).join("")}…` : characters.join("");
}

function cleanProjectName(value?: string) {
  const normalized = String(value || "")
    .replace(/\s*·\s*归档\s*$/, "")
    .trim();
  if (!normalized) return "";
  return normalized.split(/[\\/]/).filter(Boolean).at(-1)?.trim() || normalized;
}

function readableSummary(rawTitle: string) {
  const raw = String(rawTitle || "").replace(/\s+/g, " ").trim();
  if (!raw) return "详情";

  const conversationLabel = raw.match(/\[([^\]]+)]\(chatgpt-conversation:\/\/[^)]+\)/i)?.[1];
  if (conversationLabel) return excerpt(conversationLabel, 24);

  let value = raw
    .replace(/^Codex\s*任务已完成[：:]\s*/i, "")
    .replace(/^\/goal\s*/i, "")
    .replace(/\[\$?[^\]]+\]\((?:chatgpt-conversation|skill):\/\/[^)]+\)/gi, " ")
    .replace(/chatgpt-conversation:\/\/[0-9a-f-]+/gi, " ")
    .replace(/\b[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}\b/gi, " ")
    .replace(/^[\s【[(（]+|[\s】\])）]+$/g, "")
    .replace(/^(?:以此方案为基础|请帮我|帮我|需要|现在需要)[，,:：\s]*/u, "")
    .replace(/\s+/g, " ")
    .trim();
  value = value.split(/[。；;！？!?\n]/u)[0]?.trim() || value;
  return excerpt(value || "详情", 24);
}

export function compactDetailTitle(rawTitle: string, project?: string) {
  const raw = String(rawTitle || "").replace(/\s+/g, " ").trim();
  if (raw && !/(?:chatgpt-conversation:\/\/|^\/goal\b|\[[^\]]+\]\([^)]+\))/i.test(raw) && /^[^：:]{1,18}[：:]/u.test(raw)) {
    return excerpt(raw.replace(":", "："), 36);
  }
  const summary = readableSummary(raw);
  const projectName = cleanProjectName(project);
  if (!projectName || summary.toLowerCase() === projectName.toLowerCase()) return summary;
  return excerpt(`${projectName}：${summary}`, 36);
}
