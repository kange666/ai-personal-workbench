<script setup lang="ts">
import { computed, onMounted, ref, shallowRef } from "vue";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { useRoute } from "vue-router";
import ThemeSwitch from "../components/ThemeSwitch.vue";
import {
  clearDeepSeekKey,
  clearTapdCredentials,
  activateVip,
  checkForUpdates,
  createDatabaseBackup,
  deactivateVip,
  deleteQqEmailConfig,
  exportDatabaseBackup,
  getAiStatus,
  getBackupStatus,
  getEmailNotificationStatus,
  getTapdStatus,
  getVipStatus,
  getWorkTimeSettings,
  isTauriRuntime,
  restoreDatabaseBackup,
  revealLocalFile,
  saveDeepSeekKey,
  saveQqEmailConfig,
  saveTapdCredentials,
  saveWorkTimeSettings,
  testDeepSeek,
  testQqEmail,
  testTapdConnection,
  type AiStatus,
  type BackupStatus,
  type EmailNotificationStatus,
  type TapdStatus,
  type UpdateStatus,
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
const tapdStatus = ref<TapdStatus>({ configured:false, source:"未配置", authMode:"token", workspaceId:"37583308", workspaceName:"安全生产管理", owner:"刘子世康", itemCount:0, warnings:[], autoFixEnabled:false, autoFixRepositoryPath:"" });
const tapdAuthMode = ref<"token" | "basic">("token");
const tapdUser = ref("");
const tapdPassword = ref("");
const tapdAccessToken = ref("");
const tapdOwner = ref("刘子世康");
const emailStatus = ref<EmailNotificationStatus>({ configured:false, enabled:false, state:"unconfigured", maskedEmail:"", lastError:"", retryingCount:0, failedCount:0 });
const qqEmail = ref("");
const qqAuthCode = ref("");
const vipStatus = ref<VipStatus>({ active:false });
const vipCode = ref("");
const backupStatus = ref<BackupStatus>({ databasePath:"", backupDirectory:"", backups:[] });
const updateStatus = ref<UpdateStatus>({ currentVersion:"", latestVersion:"", updateAvailable:false, publishedAt:"", releaseUrl:"", installerUrl:"", portableUrl:"", checkedAt:"", message:"尚未检查" });
const pendingUpdate = shallowRef<Update | null>(null);
const updatePhase = ref<"idle" | "checking" | "ready" | "backing-up" | "downloading" | "installing" | "error">("idle");
const updateDownloaded = ref(0);
const updateTotal = ref(0);
const updateProgress = computed(() => updateTotal.value > 0 ? Math.min(100, Math.round(updateDownloaded.value / updateTotal.value * 100)) : 0);
const updateBusy = computed(() => ["checking", "backing-up", "downloading", "installing"].includes(updatePhase.value));

function emailStateText() {
  return ({ unconfigured:"未配置", unverified:"待测试", disabled:"已配置 · 已关闭", ready:"已配置 · 已开启", error:"连接异常" } as Record<string,string>)[emailStatus.value.state] || "未配置";
}

async function refresh() {
  if (!isTauriRuntime()) return;
  [status.value, tapdStatus.value, emailStatus.value, vipStatus.value, backupStatus.value, updateStatus.value] = await Promise.all([
    getAiStatus(),
    getTapdStatus(),
    getEmailNotificationStatus(),
    getVipStatus(),
    getBackupStatus(),
    checkForUpdates(),
  ]);
  tapdAuthMode.value = tapdStatus.value.authMode;
  tapdOwner.value = tapdStatus.value.owner;
  autostartEnabled.value = await isEnabled();
  workGapMinutes.value = (await getWorkTimeSettings()).gapMinutes;
}
function backupTime(value:string) { return value ? new Intl.DateTimeFormat("zh-CN", { month:"numeric", day:"numeric", hour:"2-digit", minute:"2-digit" }).format(new Date(value)) : "未知时间"; }
function backupKind(value:string) { return ({ daily:"每日自动", manual:"手工", export:"导出", "pre-restore":"恢复前保护", migration:"升级前保护" } as Record<string,string>)[value] || value; }
function backupSize(value:number) { return value >= 1024 * 1024 ? `${(value / 1024 / 1024).toFixed(1)} MB` : `${Math.max(1,Math.round(value / 1024))} KB`; }
async function createBackup() { loading.value=true; error.value=""; message.value=""; try { const backup=await createDatabaseBackup(); backupStatus.value=await getBackupStatus(); message.value=`备份已创建：${backup.fileName}`; } catch(cause) { error.value=String(cause); } finally { loading.value=false; } }
async function exportBackup() { loading.value=true; error.value=""; message.value=""; try { const backup=await exportDatabaseBackup(); message.value=`备份已导出到：${backup.path}`; await revealLocalFile(backup.path); } catch(cause) { error.value=String(cause); } finally { loading.value=false; } }
async function restoreBackup(path:string) { if (!window.confirm("恢复会用所选备份替换当前本地数据。系统会先自动创建一份“恢复前保护”备份，是否继续？")) return; loading.value=true; error.value=""; message.value=""; try { backupStatus.value=await restoreDatabaseBackup(path); message.value="恢复完成，页面即将重新载入。"; window.setTimeout(()=>window.location.reload(),800); } catch(cause) { error.value=String(cause); } finally { loading.value=false; } }
async function locateBackup(path:string) { try { await revealLocalFile(path); } catch(cause) { error.value=String(cause); } }
async function prepareSignedUpdate() {
  if (pendingUpdate.value) await pendingUpdate.value.close();
  pendingUpdate.value = null;
  updatePhase.value = "checking";
  const available = await check({ timeout: 30_000 });
  if (!available) {
    updatePhase.value = "idle";
    return null;
  }
  pendingUpdate.value = available;
  updatePhase.value = "ready";
  return available;
}
async function refreshUpdateStatus() {
  loading.value=true; error.value=""; message.value="";
  try {
    updateStatus.value=await checkForUpdates();
    if (updateStatus.value.updateAvailable) {
      const signedUpdate = await prepareSignedUpdate();
      if (!signedUpdate) throw new Error("下载页显示有新版本，但签名更新包尚未就绪，请稍后重试。");
    } else {
      updatePhase.value = "idle";
    }
  } catch(cause) {
    updatePhase.value="error";
    error.value=`检查更新失败：${String(cause)}`;
  } finally { loading.value=false; }
}
function formatUpdateBytes(value:number) { return value >= 1024 * 1024 ? `${(value / 1024 / 1024).toFixed(1)} MB` : `${Math.max(0,Math.round(value / 1024))} KB`; }
async function installAvailableUpdate() {
  error.value=""; message.value="";
  try {
    const update = pendingUpdate.value || await prepareSignedUpdate();
    if (!update) { message.value="当前已经是最新版本。"; return; }
    if (!window.confirm(`即将更新到 V${update.version}。工作台会先备份本地数据，安装时自动关闭并重新启动，是否继续？`)) return;
    updatePhase.value="backing-up";
    await createDatabaseBackup();
    updateDownloaded.value=0;
    updateTotal.value=0;
    updatePhase.value="downloading";
    await update.downloadAndInstall((event) => {
      if (event.event === "Started") updateTotal.value=event.data.contentLength || 0;
      if (event.event === "Progress") updateDownloaded.value += event.data.chunkLength;
      if (event.event === "Finished") updatePhase.value="installing";
    }, { timeout: 5 * 60_000 });
    message.value="新版本已安装，正在重新启动工作台。";
    await relaunch();
  } catch(cause) {
    updatePhase.value="error";
    error.value=`一键更新失败：${String(cause)}。当前版本和本地数据没有被替换。`;
  }
}
function openUpdateUrl(url:string) { if (url) window.open(url,"_blank","noopener,noreferrer"); }

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
    <section class="settings-section">
      <header class="settings-section-title"><div><h2>常用设置</h2><p>调整显示、工时、会员功能与本地数据。</p></div></header>
      <div class="settings-overview-grid">
        <div class="settings-stack">
          <article class="panel settings-card compact-setting-card">
            <div><h2>外观</h2><p>保持 B 指挥中心布局，仅切换颜色。</p></div><ThemeSwitch />
          </article>
          <article class="panel settings-card worktime-settings">
            <div><h2>工时估算间隔</h2><p>相邻本地活动不超过该间隔时归为同一工作区间，默认 45 分钟。</p></div>
            <label><input v-model.number="workGapMinutes" type="number" min="15" max="120"><span>分钟</span></label>
            <button class="button primary" :disabled="loading" @click="saveWorkGap">保存</button>
            <small>自动结果始终标注“估算工时”，不是精确考勤；可在工作记录中手工修正。</small>
          </article>
          <article class="panel settings-card vip-settings">
            <div><h2>VIP 功能</h2><p>启用后开放内容工坊和视频中心，状态仅保存在本机。</p></div>
            <span class="settings-status" :class="{ ready:vipStatus.active }">{{ vipStatus.active ? '已启用' : '未启用' }}</span>
            <label v-if="!vipStatus.active">VIP 码<input v-model="vipCode" type="password" inputmode="numeric" maxlength="4" autocomplete="off" placeholder="请输入 4 位 VIP 码" @keyup.enter="enableVip"></label>
            <div class="settings-actions"><button v-if="!vipStatus.active" class="button primary" :disabled="loading || vipCode.length !== 4" @click="enableVip">启用 VIP</button><button v-else class="button secondary" :disabled="loading" @click="disableVip">关闭 VIP</button></div>
          </article>
        </div>
        <div class="settings-stack">
          <article class="panel settings-card data-safety-settings">
            <div><h2>本地数据保护</h2><p>每天首次启动自动备份，保留最近 14 份；恢复前会再做一次保护备份。</p></div>
            <span class="settings-status ready">{{ backupStatus.backups.length }} 份可用</span>
            <div class="settings-actions"><button class="button primary" :disabled="loading" @click="createBackup">立即备份</button><button class="button secondary" :disabled="loading" @click="exportBackup">导出到文档</button></div>
            <div v-if="backupStatus.backups.length" class="backup-list"><article v-for="item in backupStatus.backups.slice(0,5)" :key="item.path"><span><b>{{ backupKind(item.kind) }}</b><small>{{ backupTime(item.createdAt) }} · {{ backupSize(item.sizeBytes) }}</small></span><button class="text-button" @click="locateBackup(item.path)">定位</button><button class="text-button danger-text" @click="restoreBackup(item.path)">恢复</button></article></div>
            <small v-else>尚无备份；点击“立即备份”即可创建第一份。</small>
          </article>
          <article class="panel settings-card update-settings">
            <div><h2>版本更新</h2><p>自动下载并验证官方签名；安装前先备份本地数据，完成后重新启动。</p></div>
            <span class="settings-status" :class="{ ready:!updateStatus.updateAvailable, warning:updateStatus.updateAvailable }">当前 V{{ updateStatus.currentVersion || '—' }}</span>
            <div class="update-summary"><b>{{ updateStatus.message }}</b><small v-if="updateStatus.latestVersion">线上版本 {{ updateStatus.latestVersion }}</small></div>
            <div v-if="updateBusy || updatePhase==='ready'" class="update-progress">
              <span><b>{{ updatePhase==='checking' ? '正在验证更新包' : updatePhase==='backing-up' ? '正在备份本地数据' : updatePhase==='downloading' ? '正在下载更新' : updatePhase==='installing' ? '正在安装更新' : '更新包已验证' }}</b><em v-if="updatePhase==='downloading' && updateTotal">{{ updateProgress }}%</em></span>
              <div v-if="updatePhase==='downloading'"><i :style="{width:`${updateProgress}%`}"></i></div>
              <small v-if="updatePhase==='downloading'">{{ formatUpdateBytes(updateDownloaded) }}<template v-if="updateTotal"> / {{ formatUpdateBytes(updateTotal) }}</template></small>
              <small v-else-if="updatePhase==='ready'">签名校验将在安装时再次执行。</small>
            </div>
            <div class="settings-actions"><button class="button secondary" :disabled="loading || updateBusy" @click="refreshUpdateStatus">重新检查</button><button v-if="updateStatus.updateAvailable" class="button primary" :disabled="updateBusy" @click="installAvailableUpdate">{{ updatePhase==='checking' ? '验证中…' : updatePhase==='backing-up' ? '备份中…' : updatePhase==='downloading' ? `下载 ${updateProgress}%` : updatePhase==='installing' ? '安装中…' : '一键更新并重启' }}</button><button v-if="updateStatus.updateAvailable && updateStatus.installerUrl && updatePhase==='error'" class="button secondary" @click="openUpdateUrl(updateStatus.installerUrl)">手工下载</button></div>
          </article>
        </div>
      </div>
    </section>
    <section class="settings-section">
      <header class="settings-section-title"><div><h2>外部服务</h2><p>密钥和授权信息只保存在当前电脑。</p></div></header>
      <div class="settings-service-list">
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
        <div><h2>QQ 邮件通知</h2><p>邮件开关开启后，新完成的 Codex 任务会发送到同一个 QQ 邮箱。</p></div>
        <span class="settings-status" :class="{ready:emailStatus.state==='ready',error:emailStatus.state==='error',disabled:emailStatus.state==='disabled'}">{{ emailStateText() }}</span>
        <label>QQ 邮箱<input v-model="qqEmail" type="email" autocomplete="off" :placeholder="emailStatus.maskedEmail ? `已保存：${emailStatus.maskedEmail}` : '例如 123456@qq.com'"></label>
        <label>SMTP 授权码<input v-model="qqAuthCode" type="password" autocomplete="new-password" placeholder="QQ 邮箱生成的授权码；保存后不回显"></label>
        <p v-if="emailStatus.lastError" class="email-settings-error">{{ emailStatus.lastError }}<span v-if="emailStatus.retryingCount || emailStatus.failedCount"> · 重试中 {{ emailStatus.retryingCount }} 封，失败 {{ emailStatus.failedCount }} 封</span></p>
        <div class="settings-actions"><button v-if="emailStatus.configured" class="button secondary" :disabled="loading" @click="testEmail">发送测试邮件</button><button class="button primary" :disabled="loading || !qqEmail.trim() || !qqAuthCode.trim()" @click="saveEmail">保存配置</button><button v-if="emailStatus.configured" class="button secondary danger-button" :disabled="loading" @click="clearEmail">删除配置</button></div>
        <small>固定使用 smtp.qq.com:465（SSL/TLS），发件人与收件人相同。授权码仅保存在 Windows 凭据管理器中，不会写入数据库、日志或回显到页面。</small>
      </article>
      </div>
    </section>
    <section class="settings-section">
      <header class="settings-section-title"><div><h2>自动化</h2><p>控制工作台在后台持续运行的行为。</p></div></header>
      <article class="panel settings-card report-automation-settings">
        <div><h2>自动报告</h2><p>关闭窗口后程序保留在托盘；每天 22:00 生成日报，周日和月末同时生成周报、月报。</p></div>
        <div class="autostart-control"><span class="settings-status ready">自动生成已启用</span><button class="button secondary" @click="toggleAutostart">开机启动：{{ autostartEnabled ? '开' : '关' }}</button></div>
      </article>
    </section>
  </div>
</template>
