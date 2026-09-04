<script setup lang="ts">
import type { WorkTask } from "../types/workbench";

defineProps<{ task: WorkTask }>();
const emit = defineEmits<{ toggle: [id: string]; confirm: [id: string]; postpone: [id: string]; edit: [task: WorkTask] }>();

const statusMap = { todo: "待办", doing: "进行中", done: "已完成", blocked: "阻塞", overdue: "逾期", cancelled: "已取消", draft: "待确认" } as const;
const scopeMap = { day: "每日任务", week: "每周任务", project: "项目任务" } as const;
const sourceMap = { manual: "人工", conversation: "Codex", test: "测试", report: "报告", ai: "Codex", inbox: "待办箱" } as const;
</script>

<template>
  <div class="task-row" @dblclick="emit('edit', task)">
    <button v-if="task.status === 'draft'" class="button secondary small task-confirm" title="确认后进入正式任务" @click.stop="emit('confirm', task.id)">确认</button><button v-else class="task-check" :class="{ checked: task.status === 'done' }" @click.stop="emit('toggle', task.id)">{{ task.status === "done" ? "✓" : "" }}</button>
    <div class="task-copy"><b :class="{ completed: task.status === 'done' }">{{ task.title }}</b><small>{{ task.project }} · {{ task.plannedDate || (task.weekStart ? `${task.weekStart} 所属周` : "未排期") }}</small></div>
    <span class="tag">{{ scopeMap[task.scope] }}</span><span class="tag">来源：{{ sourceMap[task.source] }}</span><span class="tag priority" :class="task.priority.toLowerCase()">{{ task.priority }}</span><span class="tag status" :class="task.status">{{ statusMap[task.status] }}</span>
    <button v-if="task.status === 'overdue'" class="more-button" title="每日任务顺延到明天；每周或项目任务顺延一周" @click.stop="emit('postpone',task.id)">顺延</button><button class="more-button" @click="emit('edit', task)">•••</button>
  </div>
</template>
