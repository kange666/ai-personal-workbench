<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { subscribeConfirm, type ConfirmRequest } from "../utils/confirm";

const request = ref<ConfirmRequest | null>(null);
const confirmButton = ref<HTMLButtonElement | null>(null);
let unsubscribe: (() => void) | null = null;

function answer(accepted: boolean) {
  const current = request.value;
  if (!current) return;
  request.value = null;
  current.respond(accepted);
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape" && request.value) answer(false);
}

onMounted(() => {
  unsubscribe = subscribeConfirm((nextRequest) => {
    request.value = nextRequest;
    void nextTick(() => confirmButton.value?.focus());
  });
  window.addEventListener("keydown", onKeydown);
});

onBeforeUnmount(() => {
  unsubscribe?.();
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <Teleport to="body">
    <Transition name="workbench-confirm">
      <div v-if="request" class="workbench-confirm-backdrop" @click.self="answer(false)">
        <section class="workbench-confirm-dialog panel" :class="request.tone" role="alertdialog" aria-modal="true" aria-labelledby="workbench-confirm-title" aria-describedby="workbench-confirm-message">
          <header>
            <i aria-hidden="true">!</i>
            <span><small>操作确认</small><h2 id="workbench-confirm-title">{{ request.title }}</h2></span>
            <button type="button" class="icon-button" title="取消" aria-label="取消" @click="answer(false)">×</button>
          </header>
          <p id="workbench-confirm-message">{{ request.message }}</p>
          <footer>
            <button type="button" class="button secondary" @click="answer(false)">{{ request.cancelText }}</button>
            <button ref="confirmButton" type="button" class="button" :class="request.tone === 'danger' ? 'danger-button' : 'primary'" @click="answer(true)">{{ request.confirmText }}</button>
          </footer>
        </section>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.workbench-confirm-backdrop{position:fixed;inset:0;z-index:1200;display:grid;place-items:center;padding:24px;background:rgba(5,8,16,.62);backdrop-filter:blur(5px)}
.workbench-confirm-dialog{width:min(460px,calc(100vw - 40px));overflow:hidden;border-color:color-mix(in srgb,var(--primary) 30%,var(--line));box-shadow:0 26px 80px rgba(0,0,0,.48)}
.workbench-confirm-dialog>header{min-height:76px;padding:16px 18px;display:grid;grid-template-columns:38px minmax(0,1fr) 34px;align-items:center;gap:12px;border-bottom:1px solid var(--line);background:linear-gradient(120deg,var(--primary-soft),transparent 62%)}
.workbench-confirm-dialog>header>i{width:36px;height:36px;border-radius:10px;display:grid;place-items:center;background:var(--primary-soft);color:var(--primary);font:800 18px/1 inherit;font-style:normal}
.workbench-confirm-dialog>header>span{min-width:0;display:grid;gap:4px}.workbench-confirm-dialog small{color:var(--muted);font-size:9px;letter-spacing:.08em}.workbench-confirm-dialog h2{margin:0;font-size:17px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.workbench-confirm-dialog>p{min-height:88px;margin:0;padding:20px 22px;color:var(--text);font-size:13px;line-height:1.75;white-space:pre-wrap;overflow-wrap:anywhere}
.workbench-confirm-dialog>footer{min-height:66px;padding:12px 18px;border-top:1px solid var(--line);display:flex;align-items:center;justify-content:flex-end;gap:9px;background:var(--surface-2)}
.workbench-confirm-dialog.warning{border-color:color-mix(in srgb,var(--warning) 44%,var(--line))}.workbench-confirm-dialog.warning>header>i{background:color-mix(in srgb,var(--warning) 13%,transparent);color:var(--warning)}
.workbench-confirm-dialog.danger{border-color:color-mix(in srgb,var(--danger) 44%,var(--line))}.workbench-confirm-dialog.danger>header>i{background:color-mix(in srgb,var(--danger) 13%,transparent);color:var(--danger)}
.workbench-confirm-dialog .danger-button{background:var(--danger);border-color:var(--danger);color:#fff}.workbench-confirm-dialog .danger-button:hover{filter:brightness(1.08)}
.workbench-confirm-enter-active,.workbench-confirm-leave-active{transition:opacity .16s ease}.workbench-confirm-enter-active .workbench-confirm-dialog,.workbench-confirm-leave-active .workbench-confirm-dialog{transition:transform .18s ease,opacity .16s ease}.workbench-confirm-enter-from,.workbench-confirm-leave-to{opacity:0}.workbench-confirm-enter-from .workbench-confirm-dialog,.workbench-confirm-leave-to .workbench-confirm-dialog{opacity:0;transform:translateY(10px) scale(.985)}
@media(max-width:560px){.workbench-confirm-backdrop{padding:14px}.workbench-confirm-dialog{width:100%}.workbench-confirm-dialog>p{min-height:72px;padding:17px}}
@media(prefers-reduced-motion:reduce){.workbench-confirm-enter-active,.workbench-confirm-leave-active,.workbench-confirm-enter-active .workbench-confirm-dialog,.workbench-confirm-leave-active .workbench-confirm-dialog{transition:none}}
</style>
