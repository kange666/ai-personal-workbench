<script setup lang="ts">
import { computed, onMounted, ref, shallowRef } from "vue";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { useRoute } from "vue-router";
import ThemeSwitch from "../components/ThemeSwitch.vue";
import NavIcon from "../components/NavIcon.vue";
import { loadHiddenNavigationPaths, loadNavigationOrder, orderedNavigationItems, saveHiddenNavigationPaths, saveNavigationOrder, workbenchNavigationItems } from "../utils/navigation";
import { confirmAction } from "../utils/confirm";
import { fontSizeOptions, loadFontSize, saveFontSize, type FontSize } from "../utils/fontSize";
import { refreshUpdateStatus as checkForUpdates } from "../services/updateStatus";
import {
  clearDeepSeekKey,
  clearApifoxToken,
  clearTapdCredentials,
  activateVip,
  createDatabaseBackup,
  deactivateVip,
  deleteQqEmailConfig,
  exportDatabaseBackup,
  getAiStatus,
  getApifoxCredentialStatus,
  getBackupStatus,
  getEmailNotificationStatus,
  getTapdStatus,
  getTrayIconStyle,
  getUpdaterProxy,
  getVipStatus,
  getWorkTimeSettings,
  isTauriRuntime,
  restoreDatabaseBackup,
  revealLocalFile,
  saveDeepSeekKey,
  saveApifoxToken,
  saveQqEmailConfig,
  saveTapdCredentials,
  setTrayIconStyle,
  saveWorkTimeSettings,
  testDeepSeek,
  testQqEmail,
  testTapdConnection,
  type AiStatus,
  type ApifoxCredentialStatus,
  type BackupStatus,
  type EmailNotificationStatus,
  type TapdStatus,
  type TrayIconStyle,
  type UpdateStatus,
  type VipStatus,
} from "../services/backend";

const route = useRoute();
const status = ref<AiStatus>({ configured: false, source: "未配置", model: "deepseek-v4-flash" });
const apifoxStatus = ref<ApifoxCredentialStatus>({ configured:false, source:"未配置" });
const apifoxToken = ref("");
const key = ref("");
const message = ref("");
const error = ref("");
const loading = ref(false);
const autostartEnabled = ref(false);
const workGapMinutes = ref(45);
const fontSize = ref(loadFontSize());
const fontSizeRevision = ref(0);
function changeFontSize(value: FontSize) {
  try {
    saveFontSize(value);
    fontSize.value = value;
  } catch {
    // 原生单选框在 change 前已切换；重建选项，让保存失败时恢复已应用的选择。
    fontSizeRevision.value += 1;
    error.value = "字号设置保存失败，请重试。";
  }
}
const tapdStatus = ref<TapdStatus>({ configured:false, source:"未配置", authMode:"token", workspaceId:"", workspaceName:"", owner:"", itemCount:0, warnings:[], autoFixEnabled:false, autoFixRepositoryPath:"", automationPaused:false, projects:[] });
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
const updateHint = ref("");
const menuOrder = ref(loadNavigationOrder());
const hiddenMenuPaths = ref(loadHiddenNavigationPaths());
const trayIconStyle = ref<TrayIconStyle>("B");
const trayStyleSaving = ref(false);
const trayStyleOptions: Array<{ value: TrayIconStyle; label: string }> = [
  { value: "A", label: "紫底" },
  { value: "B", label: "白底" },
  { value: "F", label: "荧光" },
  { value: "G", label: "纯数字" },
  { value: "H", label: "浅色数字" },
  { value: "C", label: "圆环" },
  { value: "D", label: "电量条" },
  { value: "E", label: "分段柱" },
];
let updateSlowTimer: number | undefined;
const updateProgress = computed(() => updateTotal.value > 0 ? Math.min(100, Math.round(updateDownloaded.value / updateTotal.value * 100)) : 0);
const updateBusy = computed(() => ["checking", "backing-up", "downloading", "installing"].includes(updatePhase.value));
const orderedMenuItems = computed(() => orderedNavigationItems(menuOrder.value));
const hiddenMenuPathSet = computed(() => new Set(hiddenMenuPaths.value));

function emailStateText() {
  return ({ unconfigured:"未配置", unverified:"待测试", disabled:"已配置 · 已关闭", ready:"已配置 · 已开启", error:"连接异常" } as Record<string,string>)[emailStatus.value.state] || "未配置";
}

async function refresh() {
  if (!isTauriRuntime()) return;
  [status.value, apifoxStatus.value, tapdStatus.value, emailStatus.value, vipStatus.value, backupStatus.value, updateStatus.value, trayIconStyle.value] = await Promise.all([
    getAiStatus(),
    getApifoxCredentialStatus(),
    getTapdStatus(),
    getEmailNotificationStatus(),
    getVipStatus(),
    getBackupStatus(),
    checkForUpdates(),
    getTrayIconStyle(),
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
async function restoreBackup(path:string) { if (!await confirmAction({ title:"恢复本地数据", message:"恢复会用所选备份替换当前本地数据。系统会先自动创建一份“恢复前保护”备份，是否继续？", confirmText:"恢复备份", tone:"danger" })) return; loading.value=true; error.value=""; message.value=""; try { backupStatus.value=await restoreDatabaseBackup(path); message.value="恢复完成，页面即将重新载入。"; window.setTimeout(()=>window.location.reload(),800); } catch(cause) { error.value=String(cause); } finally { loading.value=false; } }
async function locateBackup(path:string) { try { await revealLocalFile(path); } catch(cause) { error.value=String(cause); } }
async function prepareSignedUpdate() {
  updateHint.value = "";
  if (pendingUpdate.value) await pendingUpdate.value.close();
  pendingUpdate.value = null;
  updatePhase.value = "checking";
  const proxy = await getUpdaterProxy();
  let available: Update | null = null;
  let lastError: unknown;
  for (let attempt = 0; attempt < 2; attempt += 1) {
    try {
      available = await check({ timeout: 30_000, ...(proxy ? { proxy } : {}) });
      lastError = undefined;
      break;
    } catch (cause) {
      lastError = cause;
      if (attempt === 0) {
        updateHint.value = "更新服务连接不稳定，正在自动重试…";
        await new Promise(resolve => window.setTimeout(resolve, 1_200));
      }
    }
  }
  if (lastError) throw lastError;
  if (!available) {
    updatePhase.value = "idle";
    return null;
  }
  pendingUpdate.value = available;
  updatePhase.value = "ready";
  return available;
}
async function downloadSignedUpdate(update: Update) {
  let lastError: unknown;
  for (let attempt = 0; attempt < 2; attempt += 1) {
    updateDownloaded.value=0;
    updateTotal.value=0;
    try {
      await update.download((event) => {
        if (event.event === "Started") {
          updateTotal.value=event.data.contentLength || 0;
          updateHint.value="更新包已连接，正在下载并校验签名。";
        }
        if (event.event === "Progress") {
          updateDownloaded.value += event.data.chunkLength;
          updateHint.value="正在下载更新包；下载完成后将自动启动安装器。";
        }
        if (event.event === "Finished") updateHint.value="下载完成，正在准备安装。";
      }, { timeout: 2 * 60_000 });
      return;
    } catch (cause) {
      lastError = cause;
      if (attempt === 0) {
        updateHint.value="下载连接中断，正在通过系统代理自动重试…";
        await new Promise(resolve => window.setTimeout(resolve, 1_200));
      }
    }
  }
  throw lastError;
}
async function refreshUpdateStatus() {
  loading.value=true; error.value=""; message.value=""; updateHint.value="";
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
    if (!await confirmAction({ title:`更新到 V${update.version}`, message:`工作台会先备份本地数据，安装时自动关闭并重新启动。是否继续？`, confirmText:"更新并重启", tone:"warning" })) return;
    updatePhase.value="backing-up";
    await createDatabaseBackup();
    updateDownloaded.value=0;
    updateTotal.value=0;
    updateHint.value="正在连接更新服务器…";
    updatePhase.value="downloading";
    updateSlowTimer=window.setTimeout(() => {
      if (updatePhase.value === "downloading" && updateDownloaded.value === 0) {
        updateHint.value="下载源响应较慢；可继续等待，或点击“手工下载”使用浏览器下载。";
      }
    }, 15_000);
    await downloadSignedUpdate(update);
    if (updateSlowTimer) window.clearTimeout(updateSlowTimer);
    updateSlowTimer=undefined;
    updatePhase.value="installing";
    updateHint.value="安装器启动后工作台会自动退出并重新打开，请勿重复点击。";
    await update.install();
  } catch(cause) {
    if (updateSlowTimer) window.clearTimeout(updateSlowTimer);
    updateSlowTimer=undefined;
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
async function saveApifox() { loading.value=true;error.value="";message.value="";try{await saveApifoxToken(apifoxToken.value);apifoxToken.value="";await refresh();message.value="Apifox API 访问令牌已安全保存到 Windows 凭据库。";}catch(cause){error.value=String(cause);}finally{loading.value=false;} }
async function clearApifox() { loading.value=true;error.value="";message.value="";try{await clearApifoxToken();await refresh();message.value="已删除 Windows 凭据库中的 Apifox 令牌；本地接口缓存仍然保留。";}catch(cause){error.value=String(cause);}finally{loading.value=false;} }
async function toggleAutostart() { if (!isTauriRuntime()) return; if (autostartEnabled.value) await disable(); else await enable(); autostartEnabled.value = await isEnabled(); }
async function saveWorkGap() { loading.value = true; error.value = ""; message.value = ""; try { const value = await saveWorkTimeSettings(workGapMinutes.value); workGapMinutes.value = value.gapMinutes; message.value = "工时估算间隔已保存，下一次打开工时明细时自动重新估算。"; } catch (cause) { error.value=String(cause); } finally { loading.value=false; } }
async function changeTrayIconStyle(style:TrayIconStyle) { if (style===trayIconStyle.value || trayStyleSaving.value) return; trayStyleSaving.value=true; error.value=""; message.value=""; try { trayIconStyle.value=await setTrayIconStyle(style); } catch(cause) { error.value=String(cause); } finally { trayStyleSaving.value=false; } }
async function saveTapd() { loading.value=true; error.value=""; message.value=""; try { await saveTapdCredentials(tapdAuthMode.value,tapdUser.value,tapdPassword.value,tapdAccessToken.value,tapdOwner.value); tapdUser.value=""; tapdPassword.value=""; tapdAccessToken.value=""; await refresh(); message.value=`TAPD ${tapdAuthMode.value==='token'?'个人访问令牌':'OpenAPI 账号'}已安全保存到 Windows 凭据库。`; } catch(cause) { error.value=String(cause); } finally { loading.value=false; } }
async function testTapd() { loading.value=true; error.value=""; message.value=""; try { message.value=await testTapdConnection(); await refresh(); } catch(cause) { error.value=String(cause); await refresh(); } finally { loading.value=false; } }
async function clearTapd() { loading.value=true; error.value=""; message.value=""; try { await clearTapdCredentials(); await refresh(); message.value="已删除 Windows 凭据库中的 TAPD 凭据。"; } catch(cause) { error.value=String(cause); } finally { loading.value=false; } }
async function saveEmail() { loading.value=true; error.value=""; message.value=""; try { await saveQqEmailConfig(qqEmail.value,qqAuthCode.value); qqEmail.value=""; qqAuthCode.value=""; await refresh(); message.value="QQ 邮箱和 SMTP 授权码已保存到 Windows 凭据库，请发送测试邮件完成验证。"; } catch(cause) { error.value=String(cause); } finally { loading.value=false; } }
async function testEmail() { loading.value=true; error.value=""; message.value=""; try { message.value=await testQqEmail(); await refresh(); } catch(cause) { error.value=String(cause); await refresh(); } finally { loading.value=false; } }
async function clearEmail() { if (!await confirmAction({ title:"删除邮箱通知配置", message:"确定删除 QQ 邮箱通知配置吗？已发送记录不会删除。", confirmText:"删除配置", tone:"danger" })) return; loading.value=true; error.value=""; message.value=""; try { await deleteQqEmailConfig(); qqEmail.value=""; qqAuthCode.value=""; await refresh(); message.value="已删除 Windows 凭据库中的 QQ 邮件配置。"; } catch(cause) { error.value=String(cause); } finally { loading.value=false; } }
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

function moveMenu(path:string,direction:-1|1) {
  const current=[...menuOrder.value];
  const index=current.indexOf(path);
  const target=index+direction;
  if (index<0 || target<0 || target>=current.length) return;
  [current[index],current[target]]=[current[target],current[index]];
  menuOrder.value=saveNavigationOrder(current);
  message.value="菜单顺序已保存。";
}
function resetMenuOrder() {
  menuOrder.value=saveNavigationOrder(workbenchNavigationItems.map((item)=>item.path));
  hiddenMenuPaths.value=saveHiddenNavigationPaths([]);
  message.value="菜单顺序和显示状态已恢复默认。";
}
function toggleMenuVisibility(path:string) {
  const hidden=new Set(hiddenMenuPaths.value);
  if (hidden.has(path)) hidden.delete(path); else hidden.add(path);
  hiddenMenuPaths.value=saveHiddenNavigationPaths([...hidden]);
  message.value=hidden.has(path) ? "菜单已隐藏，可随时在这里恢复显示。" : "菜单已恢复显示。";
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
        <article class="panel settings-card compact-setting-card appearance-settings">
          <h2>外观</h2>
          <div class="appearance-theme-setting"><b>主题</b><ThemeSwitch /></div>
          <div class="tray-style-setting">
            <b>托盘风格</b>
            <div class="tray-style-options" role="radiogroup" aria-label="托盘风格">
              <button v-for="option in trayStyleOptions" :key="option.value" type="button" role="radio" :aria-checked="trayIconStyle===option.value" :aria-label="option.label" :title="option.label" :disabled="trayStyleSaving" :class="['tray-style-option',`style-${option.value.toLowerCase()}`,{active:trayIconStyle===option.value}]" @click="changeTrayIconStyle(option.value)">
                <svg v-if="option.value==='C'" class="tray-style-preview" viewBox="0 0 64 64" aria-hidden="true">
                  <circle cx="32" cy="32" r="24" fill="none" stroke="var(--tray-track)" stroke-width="12" />
                  <circle cx="32" cy="32" r="24" fill="none" stroke="var(--tray-progress)" stroke-width="12" pathLength="100" stroke-dasharray="57 100" transform="rotate(-90 32 32)" />
                </svg>
                <svg v-else-if="option.value==='D'" class="tray-style-preview" viewBox="0 0 64 64" aria-hidden="true">
                  <rect x="4" y="15" width="50" height="34" rx="5" fill="none" stroke="var(--tray-progress)" stroke-width="4" />
                  <path d="M59 25v14" stroke="var(--tray-progress)" stroke-width="5" />
                  <path d="M11 32h36" stroke="var(--tray-track)" stroke-width="20" />
                  <path d="M11 32h20.5" stroke="var(--tray-accent)" stroke-width="20" />
                </svg>
                <svg v-else-if="option.value==='E'" class="tray-style-preview" viewBox="0 0 64 64" aria-hidden="true">
                  <path d="M8 42v14M24 30v26M40 18v38M56 6v50" stroke="var(--tray-track)" stroke-width="10" />
                  <path d="M8 42v14M24 30v26M40 45v11" stroke="var(--tray-progress)" stroke-width="10" />
                </svg>
                <i v-else aria-hidden="true">57</i>
                <span>{{ option.label }}</span>
              </button>
            </div>
          </div>
        </article>
        <article class="panel settings-card worktime-settings">
          <div class="worktime-setting-row">
            <h2>工时估算间隔</h2>
            <div class="worktime-setting-controls">
              <label><input v-model.number="workGapMinutes" aria-label="工时估算间隔（分钟）" type="number" min="15" max="120"><span>分钟</span></label>
              <button class="button primary" :disabled="loading" @click="saveWorkGap">保存</button>
            </div>
          </div>
          <p class="worktime-setting-hint">间隔内的活动合为同一工作区间，默认 45 分钟。估算非考勤，可在工作记录中修正。</p>
          <fieldset :key="fontSizeRevision" class="font-size-setting">
            <legend>页面字号</legend>
            <div class="font-size-options">
              <label v-for="option in fontSizeOptions" :key="option.value" :class="{ active: fontSize === option.value }">
                <input type="radio" name="page-font-size" :value="option.value" :checked="fontSize === option.value" @change="changeFontSize(option.value)">
                <span>{{ option.label }}</span>
              </label>
            </div>
            <small>中为默认 · 全部页面即时生效 · 最小 10px</small>
          </fieldset>
        </article>
        <article class="panel settings-card vip-settings">
          <div><h2>VIP 功能</h2><p>启用后开放内容工坊和视频中心，状态仅保存在本机。</p></div>
          <span class="settings-status" :class="{ ready:vipStatus.active }">{{ vipStatus.active ? '已启用' : '未启用' }}</span>
          <label v-if="!vipStatus.active">VIP 码<input v-model="vipCode" type="password" inputmode="numeric" maxlength="4" autocomplete="off" placeholder="请输入 4 位 VIP 码" @keyup.enter="enableVip"></label>
          <div class="settings-actions"><button v-if="!vipStatus.active" class="button primary" :disabled="loading || vipCode.length !== 4" @click="enableVip">启用 VIP</button><button v-else class="button secondary" :disabled="loading" @click="disableVip">关闭 VIP</button></div>
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
            <small v-if="updateHint">{{ updateHint }}</small>
          </div>
          <div class="settings-actions"><button class="button secondary" :disabled="loading || updateBusy" @click="refreshUpdateStatus">重新检查</button><button v-if="updateStatus.updateAvailable" class="button primary" :disabled="updateBusy" @click="installAvailableUpdate">{{ updatePhase==='checking' ? '验证中…' : updatePhase==='backing-up' ? '备份中…' : updatePhase==='downloading' ? (updateTotal ? `下载 ${updateProgress}%` : '正在下载…') : updatePhase==='installing' ? '正在启动安装器…' : '一键更新并重启' }}</button><button v-if="updateStatus.updateAvailable && updateStatus.installerUrl && (updatePhase==='downloading' || updatePhase==='error')" class="button secondary" @click="openUpdateUrl(updateStatus.installerUrl)">手工下载</button></div>
        </article>
        <details class="panel settings-card-wide data-safety-collapsible">
          <summary><div><h2>本地数据保护</h2><p>每天首次启动自动备份，内部备份自动清理并只保留最新 10 条；恢复前会再做一次保护备份。</p></div><span class="settings-status ready">{{ backupStatus.backups.length }} 份可用</span><span class="settings-collapse-label"></span></summary>
          <div class="data-safety-body">
            <div class="settings-actions"><button class="button primary" :disabled="loading" @click="createBackup">立即备份</button><button class="button secondary" :disabled="loading" @click="exportBackup">导出到文档</button></div>
            <div v-if="backupStatus.backups.length" class="backup-list"><article v-for="item in backupStatus.backups" :key="item.path"><span><b>{{ backupKind(item.kind) }}</b><small>{{ backupTime(item.createdAt) }} · {{ backupSize(item.sizeBytes) }}</small></span><button class="text-button" @click="locateBackup(item.path)">定位</button><button class="text-button danger-text" @click="restoreBackup(item.path)">恢复</button></article></div>
            <small v-else>尚无备份；点击“立即备份”即可创建第一份。</small>
          </div>
        </details>
      </div>
    </section>
    <details class="settings-section settings-collapsible">
      <summary class="settings-section-title"><div><h2>菜单顺序与显示</h2><p>调整左侧主菜单的顺序和显示状态；设置入口固定在底部。</p></div><span class="settings-collapse-label"></span></summary>
      <article class="panel menu-order-settings">
        <header><div><h2>左侧菜单</h2><p>隐藏只影响左侧入口，不会删除功能和数据；VIP 菜单仍需启用 VIP 后才会显示。</p></div><button class="button secondary" @click="resetMenuOrder">恢复默认</button></header>
        <div class="menu-order-list"><article v-for="(item,index) in orderedMenuItems" :key="item.path" :class="{ 'is-hidden':hiddenMenuPathSet.has(item.path) }"><b>{{ index+1 }}</b><NavIcon :name="item.icon" /><span><strong>{{ item.label }}</strong><small>{{ item.vip ? 'VIP 功能' : '常用功能' }} · {{ hiddenMenuPathSet.has(item.path) ? '已隐藏' : '显示中' }}</small></span><div><button class="button small secondary menu-visibility-button" @click="toggleMenuVisibility(item.path)">{{ hiddenMenuPathSet.has(item.path) ? '显示' : '隐藏' }}</button><button class="icon-button" title="上移" :disabled="index===0" @click="moveMenu(item.path,-1)">↑</button><button class="icon-button" title="下移" :disabled="index===orderedMenuItems.length-1" @click="moveMenu(item.path,1)">↓</button></div></article></div>
      </article>
    </details>
    <details class="settings-section settings-collapsible">
      <summary class="settings-section-title"><div><h2>外部服务</h2><p>密钥和授权信息只保存在当前电脑。</p></div><span class="settings-collapse-label"></span></summary>
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
        <div><h2>TAPD OpenAPI</h2><p>一个凭据可供多个项目共用；项目、负责人和自动规则分别在 TAPD 工作与自动处理菜单中配置。</p></div>
        <span class="settings-status" :class="{ready:tapdStatus.configured}">{{ tapdStatus.configured ? `已配置 · ${tapdStatus.authMode==='token'?'个人访问令牌':'API 账号'} · ${tapdStatus.source}` : '未配置' }}</span>
        <label>认证方式<select v-model="tapdAuthMode"><option value="token">个人访问令牌（推荐）</option><option value="basic">OpenAPI 用户名和 API 密码</option></select></label>
        <label v-if="tapdAuthMode==='token'">个人访问令牌<input v-model="tapdAccessToken" type="password" autocomplete="off" placeholder="粘贴 TAPD 个人访问令牌；保存后不再回显"></label>
        <template v-else><label>API 用户名<input v-model="tapdUser" autocomplete="off" placeholder="TAPD 开放平台 API 账号"></label><label>API 密码<input v-model="tapdPassword" type="password" autocomplete="off" placeholder="保存后不再回显"></label></template>
        <div class="settings-actions"><button v-if="tapdStatus.configured" class="button secondary" :disabled="loading" @click="testTapd">测试连接</button><button v-if="tapdStatus.configured && tapdStatus.source!=='环境变量'" class="button secondary danger-button" :disabled="loading" @click="clearTapd">删除凭据</button><button class="button primary" :disabled="loading || (tapdAuthMode==='token' ? !tapdAccessToken.trim() : !tapdUser.trim() || !tapdPassword.trim())" @click="saveTapd">保存到凭据库</button></div>
        <small>令牌或密码只保存在 Windows 凭据库。工作台通过 TAPD 官方 OpenAPI 只同步缺陷，不读取任务和需求；只有人工确认完成时才会把对应缺陷回写为“已解决”。</small>
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
      <article class="panel settings-card apifox-settings">
        <div><h2>Apifox 开放 API</h2><p>用于接口文档中心只读同步多个项目的 OpenAPI 文档。</p></div>
        <span class="settings-status" :class="{ready:apifoxStatus.configured}">{{ apifoxStatus.configured ? `已配置 · ${apifoxStatus.source}` : '未配置' }}</span>
        <label>API 访问令牌<input v-model="apifoxToken" type="password" autocomplete="off" placeholder="粘贴 Apifox API 访问令牌；保存后不再回显"></label>
        <div class="settings-actions"><RouterLink class="button secondary link-button" to="/api-docs">打开接口文档</RouterLink><button v-if="apifoxStatus.configured" class="button secondary danger-button" :disabled="loading" @click="clearApifox">删除令牌</button><button class="button primary" :disabled="loading || !apifoxToken.trim()" @click="saveApifox">保存到凭据库</button></div>
        <small>令牌仅保存在 Windows 凭据库，不写入 SQLite、日志或页面。项目 ID 和同步后的脱敏接口文档保存在本机数据库。</small>
      </article>
      </div>
    </details>
    <section class="settings-section">
      <header class="settings-section-title"><div><h2>自动化</h2><p>控制工作台在后台持续运行的行为。</p></div></header>
      <article class="panel settings-card report-automation-settings">
        <div><h2>自动报告</h2><p>关闭窗口后程序保留在托盘；每天 22:00 生成日报，周日和月末同时生成周报、月报。</p></div>
        <div class="autostart-control"><span class="settings-status ready">自动生成已启用</span><button class="button secondary" @click="toggleAutostart">开机启动：{{ autostartEnabled ? '开' : '关' }}</button></div>
      </article>
    </section>
  </div>
</template>
