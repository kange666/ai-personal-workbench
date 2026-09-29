<script setup lang="ts">
import { relaunch } from "@tauri-apps/plugin-process";
import { computed, onMounted, ref, shallowRef } from "vue";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { useRoute } from "vue-router";
import ThemeSwitch from "../components/ThemeSwitch.vue";
import NavIcon from "../components/NavIcon.vue";
import { loadHiddenNavigationPaths, loadNavigationOrder, orderedNavigationItems, saveHiddenNavigationPaths, saveNavigationOrder, workbenchNavigationItems } from "../utils/navigation";
import { confirmAction } from "../utils/confirm";
import { fontSizeOptions, loadFontSize, saveFontSize, type FontSize } from "../utils/fontSize";
import { cockpitIdleMinuteOptions, loadCockpitIdleMinutes, saveCockpitIdleMinutes, type CockpitIdleMinutes } from "../utils/cockpit";
import { loadWorkbenchNotificationsEnabled, saveWorkbenchNotificationsEnabled } from "../utils/notificationPreference";
import { refreshUpdateStatus as checkForUpdates } from "../services/updateStatus";
import { backgroundRequest } from "../services/backendRequest";
import { listBackgroundTaskStatus, type BackgroundTaskStatus } from "../services/backend";
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
const cockpitIdleMinutes = ref<CockpitIdleMinutes>(loadCockpitIdleMinutes());
const fontSize = ref(loadFontSize());
const fontSizeRevision = ref(0);
const workbenchNotificationsEnabled = ref(loadWorkbenchNotificationsEnabled());
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
function toggleWorkbenchNotifications() {
  workbenchNotificationsEnabled.value = !workbenchNotificationsEnabled.value;
  saveWorkbenchNotificationsEnabled(workbenchNotificationsEnabled.value);
  message.value = workbenchNotificationsEnabled.value ? "工作台消息通知已开启。" : "工作台消息通知已关闭。";
  error.value = "";
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
const diagnostics = ref<BackgroundTaskStatus[]>([]);
const diagnosticsLoading = ref(false);
const diagnosticsError = ref("");
async function refreshDiagnostics() {
  if (!isTauriRuntime() || diagnosticsLoading.value) return;
  diagnosticsLoading.value=true; diagnosticsError.value="";
  try { diagnostics.value=await backgroundRequest(listBackgroundTaskStatus); }
  catch { diagnosticsError.value="运行诊断读取失败，请重试。"; }
  finally { diagnosticsLoading.value=false; }
}
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
type SettingsSectionKey = "appearance" | "navigation" | "work" | "services" | "notifications" | "access";
const activeSettingsSection = ref<SettingsSectionKey>("appearance");
const settingsSections: Array<{ key: SettingsSectionKey; label: string; description: string }> = [
  { key: "appearance", label: "外观", description: "主题与托盘" },
  { key: "navigation", label: "菜单管理", description: "顺序与显示" },
  { key: "work", label: "工作与自动化", description: "工时与屏保" },
  { key: "services", label: "外部服务", description: "AI、TAPD 与接口" },
  { key: "notifications", label: "通知与邮件", description: "完成提醒" },
  { key: "access", label: "功能与数据", description: "VIP、更新与备份" },
];
const activeSettingsSectionMeta = computed(() => settingsSections.find(item => item.key === activeSettingsSection.value) || settingsSections[0]);
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
const cockpitIdleStatusText = computed(() => cockpitIdleMinutes.value === null ? "已关闭自动进入" : `${cockpitIdleMinutes.value} 分钟后进入`);

function emailStateText() {
  return ({ unconfigured:"未配置", unverified:"待测试", disabled:"已配置 · 已关闭", ready:"已配置 · 已开启", error:"连接异常" } as Record<string,string>)[emailStatus.value.state] || "未配置";
}

async function refresh() {
  if (!isTauriRuntime()) return;
  // 各设置独立赋值，外部服务不可用不影响备份、字号和本地偏好显示。
  async function read<T>(name:string, request:()=>Promise<T>, assign:(value:T)=>void) {
    try { assign(await request()); }
    catch { error.value = `${error.value ? `${error.value}；` : ""}${name}读取失败，可重新进入或在卡片中重试`; }
  }
  void backgroundRequest(checkForUpdates).then(value=>{ updateStatus.value=value; }).catch(()=>{ updateHint.value="版本检查失败，可点击重新检查。"; });
  await Promise.all([
    read("AI设置",getAiStatus,value=>{status.value=value;}),
    read("接口文档设置",getApifoxCredentialStatus,value=>{apifoxStatus.value=value;}),
    read("TAPD设置",getTapdStatus,value=>{tapdStatus.value=value;tapdAuthMode.value=value.authMode;tapdOwner.value=value.owner;}),
    read("邮件设置",getEmailNotificationStatus,value=>{emailStatus.value=value;}),
    read("功能权限",getVipStatus,value=>{vipStatus.value=value;}),
    read("本地备份",getBackupStatus,value=>{backupStatus.value=value;}),
    read("托盘样式",getTrayIconStyle,value=>{trayIconStyle.value=value;}),
    read("开机启动",isEnabled,value=>{autostartEnabled.value=value;}),
    read("工时设置",getWorkTimeSettings,value=>{workGapMinutes.value=value.gapMinutes;}),
  ]);
}
function backupTime(value:string) { return value ? new Intl.DateTimeFormat("zh-CN", { month:"numeric", day:"numeric", hour:"2-digit", minute:"2-digit" }).format(new Date(value)) : "未知时间"; }
function backupKind(value:string) { return ({ daily:"每日自动", manual:"手工", export:"导出", "pre-restore":"恢复前保护", migration:"升级前保护" } as Record<string,string>)[value] || value; }
function backupSize(value:number) { return value >= 1024 * 1024 ? `${(value / 1024 / 1024).toFixed(1)} MB` : `${Math.max(1,Math.round(value / 1024))} KB`; }
async function createBackup() { loading.value=true; error.value=""; message.value=""; try { const backup=await createDatabaseBackup(); backupStatus.value=await getBackupStatus(); message.value=`备份已创建：${backup.fileName}`; } catch(cause) { error.value=String(cause); } finally { loading.value=false; } }
async function exportBackup() { loading.value=true; error.value=""; message.value=""; try { const backup=await exportDatabaseBackup(); message.value=`备份已导出到：${backup.path}`; await revealLocalFile(backup.path); } catch(cause) { error.value=String(cause); } finally { loading.value=false; } }
async function restoreBackup(path:string) {
  if (loading.value) return;
  // 确认期间也锁定入口，避免连点创建两个恢复流程。
  loading.value=true;
  if (!await confirmAction({ title:"恢复本地数据", message:"系统会先验证并升级所选备份，正式恢复前创建当前数据的保护备份。恢复完成后工作台将重新启动，是否继续？", confirmText:"恢复并重启", tone:"danger" })) { loading.value=false; return; }
  loading.value=true; error.value=""; message.value="";
  let restored=false;
  try {
    backupStatus.value=await restoreDatabaseBackup(path);
    restored=true;
    message.value="数据恢复和结构升级已完成，正在重新启动工作台。";
    await relaunch();
  } catch(cause) {
    error.value=restored ? "数据已安全恢复，但自动重启失败。请从托盘退出后重新打开工作台；重启前已暂停数据读写。" : String(cause);
  } finally { if (!restored) loading.value=false; }
}
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
function changeCockpitIdleMinutes(event: Event) {
  const value = (event.target as HTMLSelectElement).value;
  const next: CockpitIdleMinutes = value === "never" ? null : Number(value);
  try {
    saveCockpitIdleMinutes(next);
    cockpitIdleMinutes.value = next;
    message.value = next === null ? "驾驶舱屏保已关闭，仍可从左上角手动进入。" : `驾驶舱屏保已设为无操作 ${next} 分钟后自动进入。`;
    error.value = "";
  } catch {
    cockpitIdleMinutes.value = loadCockpitIdleMinutes();
    error.value = "驾驶舱屏保时间保存失败，请重试。";
  }
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

onMounted(() => {
  if (route.query.vip === "required") {
    activeSettingsSection.value = "access";
    message.value = "内容工坊和视频中心属于 VIP 功能，请先输入 VIP 码启用。";
  }
  void refresh();
});
</script>

<template>
  <div class="view settings-view">
    <header class="page-header settings-page-header"><div><h1>设置</h1><p>管理界面偏好、本机服务与数据保护</p></div></header>
    <div v-if="message || error" class="scan-message settings-message" :class="{ error: Boolean(error) }">{{ error || message }}</div>
    <div class="settings-workspace">
      <aside class="panel settings-navigation" aria-label="设置分类">
        <header><b>设置中心</b><small>选择需要调整的类别</small></header>
        <nav>
          <button v-for="item in settingsSections" :key="item.key" type="button" :class="{ active:activeSettingsSection===item.key }" :aria-current="activeSettingsSection===item.key ? 'page' : undefined" @click="activeSettingsSection=item.key">
            <i aria-hidden="true"></i><span><b>{{ item.label }}</b><small>{{ item.description }}</small></span><em aria-hidden="true">›</em>
          </button>
        </nav>
        <footer><span></span><small>设置仅保存在当前电脑</small></footer>
      </aside>

      <main class="settings-content">
        <header class="panel settings-content-header">
          <div><small>LOCAL PREFERENCES</small><h2>{{ activeSettingsSectionMeta.label }}</h2><p>{{ activeSettingsSectionMeta.description }}</p></div>
          <span class="settings-local-badge"><i></i>本机设置</span>
        </header>

        <section v-show="activeSettingsSection==='appearance'" class="settings-pane settings-pane-appearance">
          <article class="panel settings-card compact-setting-card appearance-settings">
            <div class="settings-card-heading"><div><h2>外观</h2><p>主题颜色、页面字号和托盘图标样式</p></div><span class="settings-status ready">本机偏好</span></div>
            <div class="appearance-theme-setting"><b>主题</b><ThemeSwitch /></div>
            <div class="appearance-font-size-setting"><span><b>页面字号</b><small>中为默认</small></span><div :key="fontSizeRevision" class="font-size-options" role="radiogroup" aria-label="页面字号"><label v-for="option in fontSizeOptions" :key="option.value" :class="{ active:fontSize===option.value }"><input type="radio" name="page-font-size" :value="option.value" :checked="fontSize===option.value" @change="changeFontSize(option.value)"><span>{{ option.label }}</span></label></div></div>
            <div class="tray-style-setting">
              <b>托盘风格</b>
              <div class="tray-style-options" role="radiogroup" aria-label="托盘风格">
                <button v-for="option in trayStyleOptions" :key="option.value" type="button" role="radio" :aria-checked="trayIconStyle===option.value" :aria-label="option.label" :title="option.label" :disabled="trayStyleSaving" :class="['tray-style-option',`style-${option.value.toLowerCase()}`,{active:trayIconStyle===option.value}]" @click="changeTrayIconStyle(option.value)">
                  <svg v-if="option.value==='C'" class="tray-style-preview" viewBox="0 0 64 64" aria-hidden="true"><circle cx="32" cy="32" r="24" fill="none" stroke="var(--tray-track)" stroke-width="12" /><circle cx="32" cy="32" r="24" fill="none" stroke="var(--tray-progress)" stroke-width="12" pathLength="100" stroke-dasharray="57 100" transform="rotate(-90 32 32)" /></svg>
                  <svg v-else-if="option.value==='D'" class="tray-style-preview" viewBox="0 0 64 64" aria-hidden="true"><rect x="4" y="15" width="50" height="34" rx="5" fill="none" stroke="var(--tray-progress)" stroke-width="4" /><path d="M59 25v14" stroke="var(--tray-progress)" stroke-width="5" /><path d="M11 32h36" stroke="var(--tray-track)" stroke-width="20" /><path d="M11 32h20.5" stroke="var(--tray-accent)" stroke-width="20" /></svg>
                  <svg v-else-if="option.value==='E'" class="tray-style-preview" viewBox="0 0 64 64" aria-hidden="true"><path d="M8 42v14M24 30v26M40 18v38M56 6v50" stroke="var(--tray-track)" stroke-width="10" /><path d="M8 42v14M24 30v26M40 45v11" stroke="var(--tray-progress)" stroke-width="10" /></svg>
                  <i v-else aria-hidden="true">57</i><span>{{ option.label }}</span>
                </button>
              </div>
            </div>
          </article>
        </section>

        <section v-show="activeSettingsSection==='navigation'" class="settings-pane">
          <article class="panel menu-order-settings">
            <header><div><h2>左侧菜单</h2><p>调整显示顺序；隐藏菜单不会删除功能和数据。</p></div><button class="button secondary" @click="resetMenuOrder">恢复默认</button></header>
            <div class="menu-order-list"><article v-for="(item,index) in orderedMenuItems" :key="item.path" :class="{ 'is-hidden':hiddenMenuPathSet.has(item.path) }"><b>{{ index+1 }}</b><NavIcon :name="item.icon" /><span><strong>{{ item.label }}</strong><small>{{ item.vip ? 'VIP 功能' : '常用功能' }} · {{ hiddenMenuPathSet.has(item.path) ? '已隐藏' : '显示中' }}</small></span><div><button class="button small secondary menu-visibility-button" @click="toggleMenuVisibility(item.path)">{{ hiddenMenuPathSet.has(item.path) ? '显示' : '隐藏' }}</button><button class="icon-button" title="上移" :disabled="index===0" @click="moveMenu(item.path,-1)">↑</button><button class="icon-button" title="下移" :disabled="index===orderedMenuItems.length-1" @click="moveMenu(item.path,1)">↓</button></div></article></div>
          </article>
        </section>

        <section v-show="activeSettingsSection==='work'" class="settings-pane">
          <article class="panel settings-card worktime-settings">
            <div class="worktime-setting-row"><h2>工时估算间隔</h2><div class="worktime-setting-controls"><label><input v-model.number="workGapMinutes" aria-label="工时估算间隔（分钟）" type="number" min="15" max="120"><span>分钟</span></label><button class="button primary" :disabled="loading" @click="saveWorkGap">保存</button></div></div>
            <p class="worktime-setting-hint">{{ workGapMinutes }} 分钟内活动合并；可在工作记录中修正</p>
          </article>
          <article class="panel settings-card report-automation-settings"><div><h2>自动报告</h2><p>关闭窗口后程序保留在托盘；每天 22:00 生成日报，周日和月末同时生成周报、月报。</p></div><div class="autostart-control"><span class="settings-status ready">自动生成已启用</span><button class="button secondary" @click="toggleAutostart">开机启动：{{ autostartEnabled ? '开' : '关' }}</button></div></article>
          <article class="panel settings-card report-automation-settings cockpit-automation-settings"><div><h2>驾驶舱屏保</h2><p>无操作后自动进入驾驶舱</p></div><div class="autostart-control cockpit-idle-control"><span class="settings-status" :class="{ ready:cockpitIdleMinutes!==null }">{{ cockpitIdleStatusText }}</span><label>无操作<select :value="cockpitIdleMinutes===null ? 'never' : String(cockpitIdleMinutes)" aria-label="驾驶舱屏保时间" @change="changeCockpitIdleMinutes"><option v-for="minutes in cockpitIdleMinuteOptions" :key="minutes" :value="String(minutes)">{{ minutes }} 分钟</option><option value="never">永不自动进入</option></select></label></div></article>
        </section>

        <section v-show="activeSettingsSection==='services'" class="settings-pane settings-service-list">
          <article class="panel settings-card ai-settings"><div><h2>DeepSeek</h2><p>用于报告润色和知识问答；未配置时本地规则生成仍可使用。</p></div><span class="settings-status" :class="{ ready:status.configured }">{{ status.configured ? `已配置 · ${status.source}` : '未配置' }}</span><label>模型<input :value="status.model" disabled></label><label>API Key<input v-model="key" type="password" autocomplete="off" placeholder="输入后保存；页面不会读取或显示旧密钥"></label><div class="settings-actions"><button v-if="status.configured" class="button secondary" :disabled="loading" @click="test">测试连接</button><button v-if="status.configured && status.source !== '环境变量'" class="button secondary danger-button" :disabled="loading" @click="clear">删除密钥</button><button class="button primary" :disabled="loading || !key.trim()" @click="save">保存到凭据库</button></div><small>仅发送当前报告正文或已确认知识；AI 总结独立保存，不替换原文；不上传原始日志</small></article>
          <article class="panel settings-card tapd-settings"><div><h2>TAPD OpenAPI</h2><p>一个凭据可供多个项目共用；项目、负责人和自动规则分别在 TAPD 工作与自动处理菜单中配置。</p></div><span class="settings-status" :class="{ready:tapdStatus.configured}">{{ tapdStatus.configured ? `已配置 · ${tapdStatus.authMode==='token'?'个人访问令牌':'API 账号'} · ${tapdStatus.source}` : '未配置' }}</span><label>认证方式<select v-model="tapdAuthMode"><option value="token">个人访问令牌（推荐）</option><option value="basic">OpenAPI 用户名和 API 密码</option></select></label><label v-if="tapdAuthMode==='token'">个人访问令牌<input v-model="tapdAccessToken" type="password" autocomplete="off" placeholder="粘贴 TAPD 个人访问令牌；保存后不再回显"></label><template v-else><label>API 用户名<input v-model="tapdUser" autocomplete="off" placeholder="TAPD 开放平台 API 账号"></label><label>API 密码<input v-model="tapdPassword" type="password" autocomplete="off" placeholder="保存后不再回显"></label></template><div class="settings-actions"><button v-if="tapdStatus.configured" class="button secondary" :disabled="loading" @click="testTapd">测试连接</button><button v-if="tapdStatus.configured && tapdStatus.source!=='环境变量'" class="button secondary danger-button" :disabled="loading" @click="clearTapd">删除凭据</button><button class="button primary" :disabled="loading || (tapdAuthMode==='token' ? !tapdAccessToken.trim() : !tapdUser.trim() || !tapdPassword.trim())" @click="saveTapd">保存到凭据库</button></div><small>仅同步缺陷；人工确认完成后回写</small></article>
          <article class="panel settings-card apifox-settings"><div><h2>Apifox 开放 API</h2><p>用于接口文档中心只读同步多个项目的 OpenAPI 文档。</p></div><span class="settings-status" :class="{ready:apifoxStatus.configured}">{{ apifoxStatus.configured ? `已配置 · ${apifoxStatus.source}` : '未配置' }}</span><label>API 访问令牌<input v-model="apifoxToken" type="password" autocomplete="off" placeholder="粘贴 Apifox API 访问令牌；保存后不再回显"></label><div class="settings-actions"><RouterLink class="button secondary link-button" to="/api-docs">打开接口文档</RouterLink><button v-if="apifoxStatus.configured" class="button secondary danger-button" :disabled="loading" @click="clearApifox">删除令牌</button><button class="button primary" :disabled="loading || !apifoxToken.trim()" @click="saveApifox">保存到凭据库</button></div><small>同步后的接口文档会脱敏并缓存在本机</small></article>
        </section>

        <section v-show="activeSettingsSection==='notifications'" class="settings-pane settings-notification-pane">
          <article class="panel settings-card workbench-notification-settings">
            <div><h2>工作台消息通知</h2><p>提醒项目中的普通 Codex 对话完成；自动任务和静默 Heartbeat 不提醒。</p></div>
            <button type="button" class="settings-toggle" :class="{ active:workbenchNotificationsEnabled }" role="switch" :aria-checked="workbenchNotificationsEnabled" @click="toggleWorkbenchNotifications"><i aria-hidden="true"></i><span>{{ workbenchNotificationsEnabled ? '已开启' : '已关闭' }}</span></button>
            <small>关闭后不再弹出消息；任务结果仍可在待处理收件箱中查看。</small>
          </article>
          <article class="panel settings-card email-settings"><div><h2>QQ 邮件通知</h2><p>开关开启后，仅在项目中的普通 Codex 对话完成时发送邮件。</p></div><span class="settings-status" :class="{ready:emailStatus.state==='ready',error:emailStatus.state==='error',disabled:emailStatus.state==='disabled'}">{{ emailStateText() }}</span><label>QQ 邮箱<input v-model="qqEmail" type="email" autocomplete="off" :placeholder="emailStatus.maskedEmail ? `已保存：${emailStatus.maskedEmail}` : '例如 123456@qq.com'"></label><label>SMTP 授权码<input v-model="qqAuthCode" type="password" autocomplete="new-password" placeholder="QQ 邮箱生成的授权码；保存后不回显"></label><p v-if="emailStatus.lastError" class="email-settings-error">{{ emailStatus.lastError }}<span v-if="emailStatus.retryingCount || emailStatus.failedCount"> · 重试中 {{ emailStatus.retryingCount }} 封，失败 {{ emailStatus.failedCount }} 封</span></p><div class="settings-actions"><button v-if="emailStatus.configured" class="button secondary" :disabled="loading" @click="testEmail">发送测试邮件</button><button class="button primary" :disabled="loading || !qqEmail.trim() || !qqAuthCode.trim()" @click="saveEmail">保存配置</button><button v-if="emailStatus.configured" class="button secondary danger-button" :disabled="loading" @click="clearEmail">删除配置</button></div><small>自动任务和静默 Heartbeat 不发送 · 发件人与收件人相同</small></article>
        </section>

        <section v-show="activeSettingsSection==='access'" class="settings-pane settings-access-grid">
          <article class="panel settings-card vip-settings"><div><h2>VIP 功能</h2><p>启用内容工坊和视频中心</p></div><span class="settings-status" :class="{ ready:vipStatus.active }">{{ vipStatus.active ? '已启用' : '未启用' }}</span><label v-if="!vipStatus.active">VIP 码<input v-model="vipCode" type="password" inputmode="numeric" maxlength="4" autocomplete="off" placeholder="请输入 4 位 VIP 码" @keyup.enter="enableVip"></label><div class="settings-actions"><button v-if="!vipStatus.active" class="button primary" :disabled="loading || vipCode.length !== 4" @click="enableVip">启用 VIP</button><button v-else class="button secondary" :disabled="loading" @click="disableVip">关闭 VIP</button></div></article>
          <article class="panel settings-card update-settings"><div><h2>版本更新</h2><p>验证签名后安装；安装前自动备份</p></div><span class="settings-status" :class="{ ready:!updateStatus.updateAvailable, warning:updateStatus.updateAvailable }">当前 V{{ updateStatus.currentVersion || '—' }}</span><div class="update-summary"><b>{{ updateStatus.message }}</b><small v-if="updateStatus.latestVersion">线上版本 {{ updateStatus.latestVersion }}</small></div><div v-if="updateBusy || updatePhase==='ready'" class="update-progress"><span><b>{{ updatePhase==='checking' ? '正在验证更新包' : updatePhase==='backing-up' ? '正在备份本地数据' : updatePhase==='downloading' ? '正在下载更新' : updatePhase==='installing' ? '正在安装更新' : '更新包已验证' }}</b><em v-if="updatePhase==='downloading' && updateTotal">{{ updateProgress }}%</em></span><div v-if="updatePhase==='downloading'"><i :style="{width:`${updateProgress}%`}"></i></div><small v-if="updatePhase==='downloading'">{{ formatUpdateBytes(updateDownloaded) }}<template v-if="updateTotal"> / {{ formatUpdateBytes(updateTotal) }}</template></small><small v-else-if="updatePhase==='ready'">签名校验将在安装时再次执行。</small><small v-if="updateHint">{{ updateHint }}</small></div><div class="settings-actions"><button class="button secondary" :disabled="loading || updateBusy" @click="refreshUpdateStatus">重新检查</button><button v-if="updateStatus.updateAvailable" class="button primary" :disabled="updateBusy" @click="installAvailableUpdate">{{ updatePhase==='checking' ? '验证中…' : updatePhase==='backing-up' ? '备份中…' : updatePhase==='downloading' ? (updateTotal ? `下载 ${updateProgress}%` : '正在下载…') : updatePhase==='installing' ? '正在启动安装器…' : '一键更新并重启' }}</button><button v-if="updateStatus.updateAvailable && updateStatus.installerUrl && (updatePhase==='downloading' || updatePhase==='error')" class="button secondary" @click="openUpdateUrl(updateStatus.installerUrl)">手工下载</button></div></article>
          <details class="panel settings-card-wide data-safety-collapsible" open><summary><div><h2>本地数据保护</h2><p>每日首次启动备份，保留最近 10 份</p></div><span class="settings-status ready">{{ backupStatus.backups.length }} 份可用</span><span class="settings-collapse-label"></span></summary><div class="data-safety-body"><div class="settings-actions"><button class="button primary" :disabled="loading" @click="createBackup">立即备份</button><button class="button secondary" :disabled="loading" @click="exportBackup">导出到文档</button></div><div v-if="backupStatus.backups.length" class="backup-list"><article v-for="item in backupStatus.backups" :key="item.path"><span><b>{{ backupKind(item.kind) }}</b><small>{{ backupTime(item.createdAt) }} · {{ backupSize(item.sizeBytes) }}</small></span><button class="text-button" @click="locateBackup(item.path)">定位</button><button class="text-button danger-text" :disabled="loading" @click="restoreBackup(item.path)">恢复</button></article></div><small v-else>尚无备份；点击“立即备份”即可创建第一份。</small></div></details>
          <details class="panel settings-card-wide data-safety-collapsible diagnostic-settings" @toggle="($event.target as HTMLDetailsElement).open && refreshDiagnostics()">
            <summary><div><h2>运行诊断</h2><p>本机后台任务的最近状态；日志不含对话正文、账号和令牌</p></div><span class="settings-collapse-label"></span></summary>
            <div class="data-safety-body">
              <div class="settings-actions"><button class="button secondary" :disabled="diagnosticsLoading" @click="refreshDiagnostics">{{ diagnosticsLoading ? '读取中…' : '刷新诊断' }}</button></div>
              <p v-if="diagnosticsError" role="alert">{{ diagnosticsError }}</p>
              <div v-if="diagnostics.length" class="backup-list"><article v-for="task in diagnostics" :key="task.id"><span><b>{{ task.name }} · {{ task.running ? '运行中' : task.errorMessage ? '异常' : '正常' }}</b><small>最近成功：{{ task.lastSuccessAt ? backupTime(task.lastSuccessAt) : '尚无' }} · 耗时 {{ task.durationMs }} ms</small><small v-if="task.errorMessage">{{ task.errorMessage }}</small></span></article></div>
              <small v-else-if="!diagnosticsLoading && !diagnosticsError">本次启动尚无后台任务记录。隔离审查模式不会自动执行后台任务。</small>
            </div>
          </details>
        </section>
      </main>
    </div>
  </div>
</template>

<style scoped>
.settings-view{min-width:0}
.settings-page-header{min-height:64px;margin-bottom:10px}
.settings-page-header p{margin:5px 0 0;color:var(--muted)}
.settings-message{margin-bottom:10px}
.settings-workspace{display:grid;grid-template-columns:minmax(168px,184px) minmax(0,1fr);align-items:start;gap:12px}
.settings-navigation{position:sticky;top:0;min-width:0;overflow:hidden}
.settings-navigation>header{padding:16px 14px 12px;border-bottom:1px solid var(--line);display:flex;flex-direction:column;gap:4px}
.settings-navigation>header>b{font-size:14px}
.settings-navigation>header>small,.settings-navigation>footer>small{color:var(--muted);font-size:9px;line-height:1.5}
.settings-navigation>nav{padding:8px;display:grid;gap:3px}
.settings-navigation>nav>button{width:100%;min-height:48px;padding:8px 9px;border:1px solid transparent;border-radius:8px;background:transparent;color:var(--muted);display:grid;grid-template-columns:7px minmax(0,1fr) auto;align-items:center;gap:8px;text-align:left;transition:background .16s ease,border-color .16s ease,color .16s ease}
.settings-navigation>nav>button:hover{border-color:color-mix(in srgb,var(--primary) 24%,var(--line));background:color-mix(in srgb,var(--primary) 5%,transparent);color:var(--text)}
.settings-navigation>nav>button.active{border-color:color-mix(in srgb,var(--primary) 28%,var(--line));background:var(--primary-soft);color:var(--primary)}
.settings-navigation>nav>button>i{width:6px;height:6px;border-radius:50%;background:var(--line);transition:background .16s ease,box-shadow .16s ease}
.settings-navigation>nav>button.active>i{background:var(--primary);box-shadow:0 0 0 3px color-mix(in srgb,var(--primary) 14%,transparent)}
.settings-navigation>nav>button>span{min-width:0;display:flex;flex-direction:column;gap:3px}
.settings-navigation>nav>button b{color:inherit;font-size:11px;white-space:normal;line-height:1.35}
.settings-navigation>nav>button small{color:var(--muted);font-size:8px;white-space:normal;line-height:1.35}
.settings-navigation>nav>button em{color:var(--muted);font-size:15px;font-style:normal}
.settings-navigation>footer{min-height:46px;padding:10px 13px;border-top:1px solid var(--line);display:flex;align-items:center;gap:8px}
.settings-navigation>footer>span{width:7px;height:7px;border-radius:50%;background:var(--success);box-shadow:0 0 0 3px color-mix(in srgb,var(--success) 12%,transparent)}
.settings-content{min-width:0;display:grid;gap:10px}
.settings-content-header{min-height:82px;padding:15px 17px;display:flex;align-items:center;justify-content:space-between;gap:18px;background:linear-gradient(110deg,color-mix(in srgb,var(--primary) 6%,var(--surface)),var(--surface))}
.settings-content-header>div{min-width:0}
.settings-content-header>div>small{color:var(--primary);font-size:8px;font-weight:800;letter-spacing:1.5px}
.settings-content-header h2{margin:5px 0 3px;font-size:18px}
.settings-content-header p{margin:0;color:var(--muted);font-size:9px}
.settings-local-badge{height:28px;padding:0 10px;border:1px solid var(--line);border-radius:7px;background:var(--surface-2);color:var(--muted);display:inline-flex;align-items:center;gap:7px;white-space:nowrap;font-size:9px}
.settings-local-badge>i{width:6px;height:6px;border-radius:50%;background:var(--success)}
.settings-pane{min-width:0;display:grid;gap:10px;align-content:start}
.settings-pane>.panel{width:100%;margin:0}
.settings-pane-appearance .appearance-settings{min-height:0;padding:17px;display:grid;gap:0}
.settings-card-heading{display:flex;align-items:flex-start;justify-content:space-between;gap:16px}
.settings-card-heading>div{min-width:0}
.settings-card-heading h2{margin:0 0 5px}
.settings-card-heading p{margin:0;color:var(--muted);line-height:1.55}
.settings-card-heading>.settings-status{flex:0 0 auto}
.settings-pane-appearance .settings-card-heading{margin-bottom:14px}
.settings-pane-appearance .appearance-theme-setting{min-height:48px;padding:11px 0;border-top:1px solid var(--line)}
.appearance-font-size-setting{min-width:0;padding:12px 0;border-top:1px solid var(--line);display:grid;grid-template-columns:minmax(100px,1fr) minmax(260px,360px);align-items:center;gap:14px}
.appearance-font-size-setting>span{display:flex;flex-direction:column;gap:4px}
.appearance-font-size-setting>span>b{font-size:11px;font-weight:600}
.appearance-font-size-setting>span>small{color:var(--muted);font-size:9px}
.appearance-font-size-setting .font-size-options{clear:none;width:100%}
.settings-pane-appearance .tray-style-setting{padding-top:13px;border-top:1px solid var(--line)}
.settings-pane-appearance .tray-style-options{grid-template-columns:repeat(8,minmax(0,1fr))}
.settings-pane>.worktime-settings{min-height:120px;display:flex;flex-direction:column;align-items:stretch;justify-content:center;gap:9px}
.settings-pane>.report-automation-settings{grid-column:auto}
.settings-service-list>.settings-card{grid-column:auto}
.settings-notification-pane{grid-template-columns:minmax(0,1fr)}
.settings-notification-pane>.settings-card{grid-column:1}
.workbench-notification-settings{min-height:122px;display:grid;grid-template-columns:minmax(0,1fr) auto;align-items:center;gap:10px 18px}
.workbench-notification-settings>small{grid-column:1/-1;color:var(--muted);line-height:1.5}
.settings-toggle{min-width:92px;height:36px;padding:0 10px;border:1px solid var(--line);border-radius:8px;background:var(--surface-2);color:var(--muted);display:flex;align-items:center;justify-content:center;gap:8px}
.settings-toggle>i{position:relative;width:30px;height:17px;flex:0 0 30px;border-radius:20px;background:var(--line);transition:background .16s ease}
.settings-toggle>i::after{content:"";position:absolute;left:3px;top:3px;width:11px;height:11px;border-radius:50%;background:var(--surface);transition:transform .16s ease}
.settings-toggle.active{border-color:color-mix(in srgb,var(--success) 45%,var(--line));background:color-mix(in srgb,var(--success) 8%,var(--surface-2));color:var(--success)}
.settings-toggle.active>i{background:var(--success)}
.settings-toggle.active>i::after{transform:translateX(13px);background:#fff}
.settings-toggle:focus-visible{outline:2px solid var(--primary);outline-offset:2px}
.settings-access-grid{grid-template-columns:repeat(2,minmax(0,1fr));align-items:stretch}
.settings-access-grid>.settings-card{height:100%;min-height:230px}
.settings-access-grid>.data-safety-collapsible{grid-column:1/-1}
.settings-pane .data-safety-collapsible{min-width:0;overflow:hidden}
.settings-pane .menu-order-settings{min-width:0}
.settings-pane .menu-order-list{max-height:calc(100vh - 285px);overflow:auto}
.settings-pane .backup-list{max-height:calc(100vh - 390px)}

@media(max-width:860px){
  .settings-workspace{grid-template-columns:1fr}
  .settings-navigation{position:static}
  .settings-navigation>header,.settings-navigation>footer{display:none}
  .settings-navigation>nav{grid-template-columns:repeat(3,minmax(0,1fr));gap:5px}
  .settings-navigation>nav>button{min-height:42px;grid-template-columns:5px minmax(0,1fr);padding:7px}
  .settings-navigation>nav>button small,.settings-navigation>nav>button em{display:none}
  .settings-access-grid{grid-template-columns:1fr}
  .appearance-font-size-setting{grid-template-columns:minmax(90px,1fr) minmax(240px,320px)}
  .settings-pane-appearance .tray-style-options{grid-template-columns:repeat(4,minmax(0,1fr))}
}

@media(max-width:620px){
  .settings-navigation>nav{grid-template-columns:repeat(2,minmax(0,1fr))}
  .settings-content-header{min-height:72px;padding:13px}
  .settings-local-badge{display:none}
  .settings-card-heading{align-items:flex-start;flex-direction:column;gap:8px}
  .appearance-font-size-setting{grid-template-columns:1fr;gap:8px}
  .workbench-notification-settings{grid-template-columns:1fr;justify-items:start}
  .settings-pane-appearance .tray-style-options{grid-template-columns:repeat(2,minmax(0,1fr))}
}
</style>
