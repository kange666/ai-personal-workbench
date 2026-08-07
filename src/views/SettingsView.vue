<script setup lang="ts">
import { onMounted, ref } from "vue";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { useRoute } from "vue-router";
import ThemeSwitch from "../components/ThemeSwitch.vue";
import {
  clearDeepSeekKey,
  clearTapdCredentials,
  activateVip,
  deactivateVip,
  deleteQqEmailConfig,
  getAiStatus,
  getEmailNotificationStatus,
  getTapdStatus,
  getVipStatus,
  getWorkTimeSettings,
  isTauriRuntime,
  saveDeepSeekKey,
  saveQqEmailConfig,
  saveTapdCredentials,
  saveWorkTimeSettings,
  testDeepSeek,
  testQqEmail,
  testTapdConnection,
  type AiStatus,
  type EmailNotificationStatus,
  type TapdStatus,
  type VipStatus,
} from "../services/backend";

const route = useRoute();
const status = ref<AiStatus>({ configured: false, source: "未配置", model: "deepseek-v4-flash" });
const key = ref("");
const message = ref("");
const error = ref("");
const loading = ref(false);
const autostartEnabled = ref(false);
const workGapMinutes = ref(45);
const tapdStatus = ref<TapdStatus>({ configured:false, source:"未配置", authMode:"token", workspaceId:"37583308", workspaceName:"安全生产管理", owner:"刘子世康", itemCount:0, warnings:[] });
const tapdAuthMode = ref<"token" | "basic">("token");
const tapdUser = ref("");
const tapdPassword = ref("");
const tapdAccessToken = ref("");
const tapdOwner = ref("刘子世康");
const emailStatus = ref<EmailNotificationStatus>({ configured:false, enabled:false, state:"unconfigured", maskedEmail:"", afterTime:"17:40", lastError:"", retryingCount:0, failedCount:0 });
const qqEmail = ref("");
const qqAuthCode = ref("");
const vipStatus = ref<VipStatus>({ active:false });
const vipCode = ref("");

function emailStateText() {
  return ({ unconfigured:"未配置", unverified:"待测试", disabled:"已配置 · 已关闭", ready:"已配置 · 已开启", error:"连接异常" } as Record<string,string>)[emailStatus.value.state] || "未配置";
}

async function refresh() {
  if (!isTauriRuntime()) return;
  [status.value, tapdStatus.value, emailStatus.value, vipStatus.value] = await Promise.all([
    getAiStatus(),
    getTapdStatus(),
    getEmailNotificationStatus(),
    getVipStatus(),
  ]);
  tapdAuthMode.value = tapdStatus.value.authMode;
  tapdOwner.value = tapdStatus.value.owner;
  autostartEnabled.value = await isEnabled();
  workGapMinutes.value = (await getWorkTimeSettings()).gapMinutes;
}

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
async function saveTapd() { loading.value=true; error.value=""; message.value=""; try { await saveTapdCredentials(tapdAuthMode.value,tapdUser.value,tapdPassword.value,tapdAccessToken.value,tapdOwner.value); tapdUser.value=""; tapdPassword.value=""; tapdAccessToken.value=""; await refresh(); message.value=`TAPD ${tapdAuthMode.value==='token'?'个人访问令牌':'OpenAPI 账号'}已安全保存到 Windows 凭据库。`; } catch(cause) { error.value=String(cause); } finally { loading.value=false; } }
async function testTapd() { loading.value=true; error.value=""; message.value=""; try { message.value=await testTapdConnection(); await refresh(); } catch(cause) { error.value=String(cause); await refresh(); } finally { loading.value=false; } }
async function clearTapd() { loading.value=true; error.value=""; message.value=""; try { await clearTapdCredentials(); await refresh(); message.value="已删除 Windows 凭据库中的 TAPD 凭据。"; } catch(cause) { error.value=String(cause); } finally { loading.value=false; } }
async function saveEmail() { loading.value=true; error.value=""; message.value=""; try { await saveQqEmailConfig(qqEmail.value,qqAuthCode.value); qqEmail.value=""; qqAuthCode.value=""; await refresh(); message.value="QQ 邮箱和 SMTP 授权码已保存到 Windows 凭据库，请发送测试邮件完成验证。"; } catch(cause) { error.value=String(cause); } finally { loading.value=false; } }
async function testEmail() { loading.value=true; error.value=""; message.value=""; try { message.value=await testQqEmail(); await refresh(); } catch(cause) { error.value=String(cause); await refresh(); } finally { loading.value=false; } }
async function clearEmail() { if (!window.confirm("确定删除 QQ 邮箱通知配置吗？已发送记录不会删除。")) return; loading.value=true; error.value=""; message.value=""; try { await deleteQqEmailConfig(); qqEmail.value=""; qqAuthCode.value=""; await refresh(); message.value="已删除 Windows 凭据库中的 QQ 邮件配置。"; } catch(cause) { error.value=String(cause); } finally { loading.value=false; } }
async function enableVip() {
  loading.value=true; error.value=""; message.value="";
  try { vipStatus.value=await activateVip(vipCode.value); vipCode.value=""; message.value="VIP 功能已启用，内容工坊和视频中心已显示。"; }
  catch(cause) { error.value=String(cause); }
  finally { loading.value=false; }
}
async function disableVip() {
  vipStatus.value=await deactivateVip();
  message.value="VIP 功能已关闭，内容工坊和视频中心已隐藏。";
}

onMounted(() => { if (route.query.vip === "required") message.value="内容工坊和视频中心属于 VIP 功能，请先输入 VIP 码启用。"; void refresh(); });
</script>

<template>
  <div class="view">
    <header class="page-header"><div><h1>设置</h1><p>管理界面主题、本地数据、轻量工时和外部服务</p></div></header>
    <div v-if="message || error" class="scan-message" :class="{ error: Boolean(error) }">{{ error || message }}</div>
    <section class="settings-grid">
      <article class="panel settings-card">
        <div><h2>外观</h2><p>保持 B 指挥中心布局，仅切换颜色。</p></div><ThemeSwitch />
      </article>
      <article class="panel settings-card vip-settings">
        <div><h2>VIP 功能</h2><p>启用后开放内容工坊和视频中心，状态仅保存在本机。</p></div>
        <span class="settings-status" :class="{ ready:vipStatus.active }">{{ vipStatus.active ? '已启用' : '未启用' }}</span>
        <label v-if="!vipStatus.active">VIP 码<input v-model="vipCode" type="password" inputmode="numeric" maxlength="4" autocomplete="off" placeholder="请输入 4 位 VIP 码" @keyup.enter="enableVip"></label>
        <div class="settings-actions"><button v-if="!vipStatus.active" class="button primary" :disabled="loading || vipCode.length !== 4" @click="enableVip">启用 VIP</button><button v-else class="button secondary" :disabled="loading" @click="disableVip">关闭 VIP</button></div>
      </article>
      <article class="panel settings-card worktime-settings">
        <div><h2>工时估算间隔</h2><p>相邻本地活动不超过该间隔时归为同一工作区间，默认 45 分钟。</p></div>
        <label><input v-model.number="workGapMinutes" type="number" min="15" max="120"><span>分钟</span></label>
        <button class="button primary" :disabled="loading" @click="saveWorkGap">保存</button>
        <small>自动结果始终标注“估算工时”，不是精确考勤；可在工作记录中手工修正。</small>
      </article>
      <article class="panel settings-card ai-settings">
        <div><h2>DeepSeek</h2><p>用于报告润色和知识问答；未配置时本地规则生成仍可使用。</p></div>
        <span class="settings-status" :class="{ ready: status.configured }">{{ status.configured ? `已配置 · ${status.source}` : '未配置' }}</span>
        <label>模型<input :value="status.model" disabled></label>
        <label>API Key<input v-model="key" type="password" autocomplete="off" placeholder="输入后保存；页面不会读取或显示旧密钥"></label>
        <div class="settings-actions"><button v-if="status.configured" class="button secondary" :disabled="loading" @click="test">测试连接</button><button v-if="status.configured && status.source !== '环境变量'" class="button secondary danger-button" :disabled="loading" @click="clear">删除密钥</button><button class="button primary" :disabled="loading || !key.trim()" @click="save">保存到凭据库</button></div>
        <small>点击“AI 润色”时会发送报告草稿与同期 Codex 对话摘录；知识问答只发送已确认知识。原始日志不会在后台自动上传。</small>
      </article>
      <article class="panel settings-card tapd-settings">
        <div><h2>TAPD · 安全生产管理</h2><p>同步项目 37583308 中“刘子世康”负责的需求、任务和缺陷。</p></div>
        <span class="settings-status" :class="{ready:tapdStatus.configured}">{{ tapdStatus.configured ? `已配置 · ${tapdStatus.authMode==='token'?'个人访问令牌':'API 账号'} · ${tapdStatus.source}` : '未配置' }}</span>
        <label>认证方式<select v-model="tapdAuthMode"><option value="token">个人访问令牌（推荐）</option><option value="basic">OpenAPI 用户名和 API 密码</option></select></label>
        <label v-if="tapdAuthMode==='token'">个人访问令牌<input v-model="tapdAccessToken" type="password" autocomplete="off" placeholder="粘贴 TAPD 个人访问令牌；保存后不再回显"></label>
        <template v-else><label>API 用户名<input v-model="tapdUser" autocomplete="off" placeholder="TAPD 开放平台 API 账号"></label><label>API 密码<input v-model="tapdPassword" type="password" autocomplete="off" placeholder="保存后不再回显"></label></template>
        <label>工作项负责人<input v-model="tapdOwner" autocomplete="off" placeholder="例如：刘子世康"></label>
        <div class="settings-actions"><button v-if="tapdStatus.configured" class="button secondary" :disabled="loading" @click="testTapd">测试连接</button><button v-if="tapdStatus.configured && tapdStatus.source!=='环境变量'" class="button secondary danger-button" :disabled="loading" @click="clearTapd">删除凭据</button><button class="button primary" :disabled="loading || (tapdAuthMode==='token' ? !tapdAccessToken.trim() : !tapdUser.trim() || !tapdPassword.trim())" @click="saveTapd">保存到凭据库</button></div>
        <small>令牌或密码只保存在 Windows 凭据库。工作台通过 TAPD 官方 OpenAPI 只读拉取内容，不读取浏览器登录状态，也不会自动回写 TAPD。</small>
      </article>
      <article class="panel settings-card email-settings">
        <div><h2>QQ 邮件通知</h2><p>Codex 任务在北京时间 {{ emailStatus.afterTime }} 以后完成时，发送到同一个 QQ 邮箱。</p></div>
        <span class="settings-status" :class="{ready:emailStatus.state==='ready',error:emailStatus.state==='error',disabled:emailStatus.state==='disabled'}">{{ emailStateText() }}</span>
        <label>QQ 邮箱<input v-model="qqEmail" type="email" autocomplete="off" :placeholder="emailStatus.maskedEmail ? `已保存：${emailStatus.maskedEmail}` : '例如 123456@qq.com'"></label>
        <label>SMTP 授权码<input v-model="qqAuthCode" type="password" autocomplete="new-password" placeholder="QQ 邮箱生成的授权码；保存后不回显"></label>
        <p v-if="emailStatus.lastError" class="email-settings-error">{{ emailStatus.lastError }}<span v-if="emailStatus.retryingCount || emailStatus.failedCount"> · 重试中 {{ emailStatus.retryingCount }} 封，失败 {{ emailStatus.failedCount }} 封</span></p>
        <div class="settings-actions"><button v-if="emailStatus.configured" class="button secondary" :disabled="loading" @click="testEmail">发送测试邮件</button><button class="button primary" :disabled="loading || !qqEmail.trim() || !qqAuthCode.trim()" @click="saveEmail">保存配置</button><button v-if="emailStatus.configured" class="button secondary danger-button" :disabled="loading" @click="clearEmail">删除配置</button></div>
        <small>固定使用 smtp.qq.com:465（SSL/TLS），发件人与收件人相同。授权码仅保存在 Windows 凭据管理器中，不会写入数据库、日志或回显到页面。</small>
      </article>
      <article class="panel settings-card report-automation-settings">
        <div><h2>自动报告</h2><p>关闭窗口后程序保留在托盘；每天 22:00 生成日报，周日和月末同时生成周报、月报。</p></div>
        <div class="autostart-control"><span class="settings-status ready">自动生成已启用</span><button class="button secondary" @click="toggleAutostart">开机启动：{{ autostartEnabled ? '开' : '关' }}</button></div>
      </article>
    </section>
  </div>
</template>
