<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { RouterLink } from "vue-router";
import { isTauriRuntime, translateText, type TranslationCandidate, type TranslationDirection } from "../services/backend";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ close: [] }>();

const CHARACTER_LIMIT = 5000;
const sourceText = ref("");
const translations = ref<TranslationCandidate[]>([]);
const manualDirection = ref<TranslationDirection | null>(null);
const loading = ref(false);
const error = ref("");
const copyMessage = ref("");
let requestSequence = 0;

const containsChinese = computed(() => /[\u3400-\u4dbf\u4e00-\u9fff]/u.test(sourceText.value));
const containsEnglish = computed(() => /[A-Za-z]/u.test(sourceText.value));
const detectedDirection = computed<TranslationDirection>(() => containsChinese.value ? "zh-to-en" : "en-to-zh");
const direction = computed<TranslationDirection>(() => manualDirection.value || detectedDirection.value);
const sourceLanguage = computed(() => direction.value === "zh-to-en" ? "中文" : "英文");
const targetLanguage = computed(() => direction.value === "zh-to-en" ? "英文" : "中文");
const characterCount = computed(() => Array.from(sourceText.value).length);
const needsDeepSeekSetup = computed(() => error.value.includes("尚未配置 DeepSeek API Key"));

function clearResult() {
  requestSequence += 1;
  loading.value = false;
  translations.value = [];
  error.value = "";
  copyMessage.value = "";
}

function reset() {
  clearResult();
  sourceText.value = "";
  manualDirection.value = null;
}

function close() {
  reset();
  emit("close");
}

function switchDirection() {
  manualDirection.value = direction.value === "zh-to-en" ? "en-to-zh" : "zh-to-en";
  clearResult();
}

function restoreAutomaticDirection() {
  manualDirection.value = null;
  clearResult();
}

function validate(): string {
  if (!sourceText.value.trim()) return "请输入需要翻译的内容。";
  if (characterCount.value > CHARACTER_LIMIT) return `翻译内容不能超过 ${CHARACTER_LIMIT} 个字符。`;
  if (!containsChinese.value && !containsEnglish.value) return "请输入中文或英文内容。";
  if (!isTauriRuntime()) return "翻译功能需要在工作台桌面版中使用。";
  return "";
}

async function translate() {
  if (loading.value) return;
  clearResult();
  const validationError = validate();
  if (validationError) {
    error.value = validationError;
    return;
  }
  const requestId = ++requestSequence;
  loading.value = true;
  try {
    const result = await translateText(sourceText.value.trim(), direction.value);
    if (requestId === requestSequence) translations.value = result;
  } catch (cause) {
    if (requestId === requestSequence) error.value = String(cause);
  } finally {
    if (requestId === requestSequence) loading.value = false;
  }
}

function handleSourceKeydown(event: KeyboardEvent) {
  if (event.key !== "Enter" || event.shiftKey || event.isComposing) return;
  event.preventDefault();
  void translate();
}

async function copyTranslation(candidate = translations.value[0]) {
  if (!candidate) return;
  try {
    await navigator.clipboard.writeText(candidate.text);
    copyMessage.value = `已复制：${candidate.label}`;
  } catch (cause) {
    error.value = `复制失败：${String(cause)}`;
  }
}

watch(sourceText, clearResult);
watch(() => props.open, (open) => {
  if (!open) {
    reset();
    return;
  }
  void nextTick(() => document.querySelector<HTMLTextAreaElement>(".translation-source textarea")?.focus());
}, { immediate: true });
</script>

<template>
  <div v-if="open" class="modal-backdrop translation-backdrop" @click.self="close">
    <section class="panel translation-dialog" role="dialog" aria-modal="true" aria-labelledby="translation-dialog-title">
      <header>
        <div><h2 id="translation-dialog-title">中英翻译</h2><p>仅支持中文与英文互译，不保存翻译记录。</p></div>
        <button class="icon-button" title="关闭" aria-label="关闭翻译" @click="close">×</button>
      </header>

      <div class="translation-body">
        <p class="translation-privacy">只有点击“翻译”后，输入内容才会发送给 DeepSeek。</p>
        <div class="translation-direction" :class="{ manual:Boolean(manualDirection) }">
          <span><small>{{ manualDirection ? '手动方向' : sourceText.trim() ? '自动识别' : '等待输入' }}</small><b>{{ sourceLanguage }}</b></span>
          <button class="translation-swap" title="交换翻译方向" aria-label="交换翻译方向" @click="switchDirection">⇄</button>
          <span><small>目标语言</small><b>{{ targetLanguage }}</b></span>
          <button v-if="manualDirection" class="text-button" @click="restoreAutomaticDirection">恢复自动识别</button>
        </div>

        <label class="translation-field translation-source">
          <span><b>原文</b><small :class="{ over:characterCount > CHARACTER_LIMIT }">{{ characterCount }} / {{ CHARACTER_LIMIT }}</small></span>
          <textarea v-model="sourceText" rows="5" placeholder="输入需要翻译的中文或英文…" @keydown="handleSourceKeydown"></textarea>
        </label>

        <section class="translation-field translation-result">
          <span><b>译文候选</b><small>{{ loading ? '正在翻译…' : translations.length ? `${translations.length} 个${targetLanguage}结果` : '等待翻译' }}</small></span>
          <div class="translation-results" :class="{ empty:!translations.length }">
            <p v-if="loading" class="translation-result-placeholder">DeepSeek 正在匹配不同语境的译法…</p>
            <p v-else-if="!translations.length" class="translation-result-placeholder">候选译文会显示在这里</p>
            <template v-else>
              <article v-for="(candidate,index) in translations" :key="`${candidate.label}:${candidate.text}`" class="translation-candidate">
                <header><span><i>{{ index + 1 }}</i><b>{{ candidate.label }}</b></span><button class="text-button" @click="copyTranslation(candidate)">复制</button></header>
                <p>{{ candidate.text }}</p>
              </article>
            </template>
          </div>
        </section>

        <p v-if="error" class="form-error translation-error">
          <span>{{ error }}</span>
          <RouterLink v-if="needsDeepSeekSetup" to="/settings" @click="close">前往设置</RouterLink>
        </p>
      </div>

      <footer>
        <span>{{ copyMessage || 'Enter 翻译 · Shift + Enter 换行' }}</span>
        <div>
          <button class="button secondary" :disabled="loading || !sourceText" @click="reset">清空</button>
          <button class="button secondary" :disabled="!translations.length" @click="copyTranslation()">复制首选</button>
          <button class="button primary" :disabled="loading || !sourceText.trim()" @click="translate">{{ loading ? '翻译中…' : '翻译' }}</button>
        </div>
      </footer>
    </section>
  </div>
</template>
