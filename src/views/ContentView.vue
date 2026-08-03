<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { generateDailyContent, isTauriRuntime, listContentIdeas, updateContentStatus, type ContentIdea } from "../services/backend";

const route = useRoute();
const router = useRouter();
const today = new Date().toLocaleDateString("sv-SE");
const selectedDate = ref(typeof route.query.date === "string" ? route.query.date : today);
const ideas = ref<ContentIdea[]>([]);
const selectedId = ref(typeof route.query.idea === "string" ? route.query.idea : "");
const loading = ref(false);
const message = ref("");
const error = ref("");
const activeSection = ref<"script" | "storyboard" | "visuals" | "editing">("script");

const selected = computed(() => ideas.value.find(item => item.id === selectedId.value) || ideas.value[0]);
const selectedCount = computed(() => ideas.value.filter(item => item.status === "selected" || item.status === "published").length);
const statusLabel: Record<ContentIdea["status"], string> = { candidate: "候选", selected: "已选择", rejected: "已淘汰", published: "已发布" };

function fallbackIdeas(date: string): ContentIdea[] {
  return Array.from({ length: 5 }, (_, index) => ({ id: `preview-${index}`, ideaDate: date, category: ["AI未来", "智能硬件", "未来生活", "科技趋势", "个人科技升级"][index], title: ["如果 AI 开始替你管理一天，会发生什么？", "AI 眼镜真的会成为下一部手机吗？", "2035 年的普通家庭，可能已经不需要开关了", "为什么所有科技公司突然都在做机器人？", "2030 年的办公桌，会变成什么样？"][index], hook: "未来最懂你的人，可能不是人。", script: "桌面预览模式不会写入数据。请从开发版桌面程序打开内容工坊，程序会自动生成当天 5 套完整内容。", storyboard: "| 时间 | 画面 | 字幕 |\n|---|---|---|\n| 0-3秒 | 未来科技轮廓 | 变化已经开始 |", visualPrompts: "写实电影感、近未来、竖屏 9:16、无品牌 Logo。", editingGuide: "总时长 55—60 秒，前 3 秒快速建立悬念。", coverTitle: "提前看见未来", status: "candidate", source: "local", createdAt: new Date().toISOString(), updatedAt: new Date().toISOString() }));
}

async function load() {
  loading.value = true;
  error.value = "";
  try {
    if (!isTauriRuntime()) ideas.value = fallbackIdeas(selectedDate.value);
    else {
      ideas.value = await listContentIdeas(selectedDate.value);
      if (ideas.value.length < 5) ideas.value = await generateDailyContent(selectedDate.value, false, false);
    }
    if (!ideas.value.some(item => item.id === selectedId.value)) selectedId.value = ideas.value[0]?.id || "";
  } catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}

async function regenerate() {
  if (!isTauriRuntime()) return;
  if ((selectedCount.value > 0) && !window.confirm("已选择的内容会保留，其余候选会重新生成。是否继续？")) return;
  loading.value = true;
  message.value = "";
  error.value = "";
  try {
    ideas.value = await generateDailyContent(selectedDate.value, true, true);
    selectedId.value = ideas.value[0]?.id || "";
    message.value = ideas.value.some(item => item.source === "deepseek") ? "已使用 DeepSeek 生成今日 5 套内容。" : "未连接 DeepSeek，已使用本地方案生成今日 5 套内容。";
  } catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}

async function setStatus(status: ContentIdea["status"]) {
  if (!selected.value || !isTauriRuntime()) return;
  await updateContentStatus(selected.value.id, status);
  selected.value.status = status;
  message.value = status === "selected" ? "已加入你的待制作列表。" : status === "rejected" ? "已淘汰这条候选。" : "状态已更新。";
}

async function copyPackage() {
  if (!selected.value) return;
  const item = selected.value;
  const text = `# ${item.title}\n\n分类：${item.category}\n封面：${item.coverTitle}\n\n## 3秒钩子\n${item.hook}\n\n## 完整口播\n${item.script}\n\n## 分镜脚本\n${item.storyboard}\n\n## AI画面提示词\n${item.visualPrompts}\n\n## 剪辑指导\n${item.editingGuide}`;
  await navigator.clipboard.writeText(text);
  message.value = "完整制作包已复制。";
}
async function copyVideoRequest() {
  if (!selected.value) return;
  const text=`标题：《${selected.value.title}》\n请使用 generate-tech-short-video 技能，基于已选择的标题制作完整科技探索竖屏短视频，并交付脚本、画面、配音、字幕、封面、MP4 和质检报告。`;
  await navigator.clipboard.writeText(text); message.value="完整视频制作请求已复制，可直接粘贴到新的 Codex 任务。";
}

function choose(item: ContentIdea) {
  selectedId.value = item.id;
  activeSection.value = "script";
  void router.replace({ query: { date: selectedDate.value, idea: item.id } });
}

watch(selectedDate, async () => { await router.replace({ query: { date: selectedDate.value } }); selectedId.value = ""; await load(); });
watch(() => route.query.idea, (id) => { if (typeof id === "string" && ideas.value.some(item => item.id === id)) selectedId.value = id; });
onMounted(load);
</script>

<template>
  <div class="view content-view">
    <header class="page-header"><div><h1>内容工坊</h1><p>每天 5 个“小众科技探索”选题，完整内容可直接进入制作</p></div><div><input v-model="selectedDate" class="button secondary content-date" type="date"><button class="button primary" :disabled="loading" @click="regenerate">{{ loading ? '生成中…' : '✦ 重新生成 5 条' }}</button></div></header>
    <p v-if="message" class="scan-message">{{ message }}</p><p v-if="error" class="scan-message error">{{ error }}</p>
    <section class="content-summary panel"><div><b>{{ ideas.length }}</b><span>今日候选</span></div><div><b>{{ selectedCount }}</b><span>已选择</span></div><div><b>{{ ideas.filter(item => item.status === 'rejected').length }}</b><span>已淘汰</span></div><p><strong>内容边界</strong><span>不伪装开箱或亲身体验；趋势与推测会明确表达。</span></p></section>
    <section class="content-layout">
      <aside class="panel content-list"><header><b>{{ selectedDate }} 候选标题</b><small>点击查看完整制作包</small></header><button v-for="(item,index) in ideas" :key="item.id" :class="[item.status,{ active:selected?.id === item.id }]" @click="choose(item)"><i>{{ index + 1 }}</i><span><small>{{ item.category }} · {{ item.source === 'deepseek' ? 'AI 生成' : '本地生成' }}</small><b>{{ item.title }}</b><em>{{ statusLabel[item.status] }}</em></span></button><p v-if="!ideas.length && !loading" class="panel-empty">当天还没有候选内容。</p></aside>
      <main v-if="selected" class="panel content-detail">
        <header><div><span>{{ selected.category }}</span><h2>{{ selected.title }}</h2><p>封面标题：<b>{{ selected.coverTitle }}</b></p></div><div><button class="button secondary" @click="setStatus(selected.status === 'candidate' ? 'rejected' : 'candidate')">{{ selected.status === 'candidate' ? '淘汰' : '恢复候选' }}</button><button class="button primary" :disabled="selected.status === 'selected'" @click="setStatus('selected')">{{ selected.status === 'selected' ? '✓ 已选择' : '✓ 选择这条' }}</button><button v-if="selected.status === 'selected'" class="button primary" @click="copyVideoRequest">继续制作完整视频</button><button class="button secondary" @click="copyPackage">复制全部</button></div></header>
        <blockquote><small>3 秒钩子</small><b>{{ selected.hook }}</b></blockquote>
        <nav class="content-tabs"><button :class="{active:activeSection==='script'}" @click="activeSection='script'">完整口播</button><button :class="{active:activeSection==='storyboard'}" @click="activeSection='storyboard'">分镜脚本</button><button :class="{active:activeSection==='visuals'}" @click="activeSection='visuals'">AI 画面</button><button :class="{active:activeSection==='editing'}" @click="activeSection='editing'">剪辑指导</button></nav>
        <article class="content-copy"><pre v-if="activeSection==='script'">{{ selected.script }}</pre><pre v-else-if="activeSection==='storyboard'">{{ selected.storyboard }}</pre><pre v-else-if="activeSection==='visuals'">{{ selected.visualPrompts }}</pre><pre v-else>{{ selected.editingGuide }}</pre></article>
      </main>
    </section>
  </div>
</template>
