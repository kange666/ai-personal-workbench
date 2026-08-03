<script setup lang="ts">
import { onMounted, ref } from "vue";
import ThemeSwitch from "../components/ThemeSwitch.vue";
import { clearDeepSeekKey, getAiStatus, getWorkTimeSettings, isTauriRuntime, saveDeepSeekKey, saveWorkTimeSettings, testDeepSeek, type AiStatus } from "../services/backend";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";

const status = ref<AiStatus>({ configured: false, source: "未配置", model: "deepseek-v4-flash" });
const key = ref("");
const message = ref("");
const error = ref("");
const loading = ref(false);
const autostartEnabled = ref(false);
const workGapMinutes = ref(45);
async function refresh() { if (isTauriRuntime()) { status.value = await getAiStatus(); autostartEnabled.value = await isEnabled(); workGapMinutes.value = (await getWorkTimeSettings()).gapMinutes; } }
async function save() {
  if (!isTauriRuntime()) { error.value = "请在桌面端配置密钥。"; return; }
  loading.value = true; error.value = ""; message.value = "";
  try { await saveDeepSeekKey(key.value); key.value = ""; await refresh(); message.value = "密钥已安全保存到 Windows 凭据库。"; }
  catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}
async function test() {
  loading.value = true; error.value = ""; message.value = "";
  try { message.value = await testDeepSeek(); }
  catch (cause) { error.value = String(cause); }
  finally { loading.value = false; }
}
async function clear() { await clearDeepSeekKey(); await refresh(); message.value = "已删除 Windows 凭据库中的 DeepSeek 密钥。"; }
async function toggleAutostart() { if (!isTauriRuntime()) return; if (autostartEnabled.value) await disable(); else await enable(); autostartEnabled.value = await isEnabled(); }
async function saveWorkGap() { loading.value = true; error.value = ""; message.value = ""; try { const value = await saveWorkTimeSettings(workGapMinutes.value); workGapMinutes.value = value.gapMinutes; message.value = "工时估算间隔已保存，下一次打开工时明细时自动重新估算。"; } catch (cause) { error.value=String(cause); } finally { loading.value=false; } }
onMounted(() => { void refresh(); });
</script>

<template><div class="view"><header class="page-header"><div><h1>设置</h1><p>管理界面主题、本地数据、轻量工时和 AI 服务</p></div></header><div v-if="message || error" class="scan-message" :class="{ error: Boolean(error) }">{{ error || message }}</div><section class="settings-grid"><article class="panel settings-card"><div><h2>外观</h2><p>保持 B 指挥中心布局，仅切换颜色。</p></div><ThemeSwitch /></article><article class="panel settings-card worktime-settings"><div><h2>工时估算间隔</h2><p>相邻本地活动不超过该间隔时归为同一工作区间，默认 45 分钟。</p></div><label><input v-model.number="workGapMinutes" type="number" min="15" max="120"><span>分钟</span></label><button class="button primary" :disabled="loading" @click="saveWorkGap">保存</button><small>自动结果始终标注“估算工时”，不是精确考勤；可在工作记录中手工修正。</small></article><article class="panel settings-card ai-settings"><div><h2>DeepSeek</h2><p>用于报告润色和知识问答；未配置时本地规则生成仍可使用。</p></div><span class="settings-status" :class="{ ready: status.configured }">{{ status.configured ? `已配置 · ${status.source}` : '未配置' }}</span><label>模型<input :value="status.model" disabled></label><label>API Key<input v-model="key" type="password" autocomplete="off" placeholder="输入后保存；页面不会读取或显示旧密钥"></label><div class="settings-actions"><button v-if="status.configured" class="button secondary" :disabled="loading" @click="test">测试连接</button><button v-if="status.configured && status.source !== '环境变量'" class="button secondary danger-button" :disabled="loading" @click="clear">删除密钥</button><button class="button primary" :disabled="loading || !key.trim()" @click="save">保存到凭据库</button></div><small>点击“AI 润色”时会发送报告草稿与同期 Codex 对话摘录；知识问答只发送已确认知识。原始日志不会在后台自动上传。</small></article><article class="panel settings-card"><div><h2>自动报告</h2><p>关闭窗口后程序保留在托盘；每天 22:00 生成日报，周日和月末同时生成周报、月报。</p></div><div class="autostart-control"><span class="settings-status ready">自动生成已启用</span><button class="button secondary" @click="toggleAutostart">开机启动：{{ autostartEnabled ? '开' : '关' }}</button></div></article></section></div></template>
