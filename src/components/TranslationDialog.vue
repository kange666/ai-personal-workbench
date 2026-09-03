<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { RouterLink } from "vue-router";
import { isTauriRuntime, translateError, translateText, type ErrorTranslation, type TranslationCandidate, type TranslationDirection } from "../services/backend";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ close: [] }>();

const CHARACTER_LIMIT = 5000;
const sourceText = ref("");
const translations = ref<TranslationCandidate[]>([]);
const errorMode = ref(false);
const errorTranslation = ref<ErrorTranslation | null>(null);
const manualDirection = ref<TranslationDirection | null>(null);
const loading = ref(false);
const error = ref("");
const copyMessage = ref("");
const copiedKey = ref("");
let requestSequence = 0;

const containsChinese = computed(() => /[\u3400-\u4dbf\u4e00-\u9fff]/u.test(sourceText.value));
const containsEnglish = computed(() => /[A-Za-z]/u.test(sourceText.value));
const detectedDirection = computed<TranslationDirection>(() => containsChinese.value ? "zh-to-en" : "en-to-zh");
const direction = computed<TranslationDirection>(() => manualDirection.value || detectedDirection.value);
const sourceLanguage = computed(() => direction.value === "zh-to-en" ? "中文" : "英文");
const targetLanguage = computed(() => direction.value === "zh-to-en" ? "英文" : "中文");
const characterCount = computed(() => Array.from(sourceText.value).length);
const needsDeepSeekSetup = computed(() => error.value.includes("尚未配置 DeepSeek API Key"));
const displayCandidates = computed(() => translations.value.map(candidate => ({
  ...candidate,
  variants: direction.value === "zh-to-en" ? namingVariants(candidate.text) : [],
})));

function namingVariants(text: string) {
  const phrase = text.trim();
  // 只转换英文短语，避免把长段落、URL 或代码误当成变量名。
  if (phrase.length > 120 || !/^[A-Za-z][A-Za-z0-9 _-]*$/.test(phrase)) return [];
  const words = phrase
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2")
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .toLowerCase().split(/[ _-]+/).filter(Boolean);
  if (words.length > 12) return [];
  const capitalized = words.map(word => word.charAt(0).toUpperCase() + word.slice(1));
  return [
    { id: "camel", label: "小驼峰", text: words[0] + capitalized.slice(1).join("") },
    { id: "pascal", label: "大驼峰", text: capitalized.join("") },
    { id: "snake", label: "下划线", text: words.join("_") },
    { id: "kebab", label: "短横线", text: words.join("-") },
  ];
}

function clearResult() {
  requestSequence += 1;
  loading.value = false;
  translations.value = [];
  errorTranslation.value = null;
  error.value = "";
  copyMessage.value = "";
  copiedKey.value = "";
}

function reset() {
  clearResult();
  sourceText.value = "";
  manualDirection.value = null;
}

function close() {
  reset();
  errorMode.value = false;
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
    if (errorMode.value) {
      const result = await translateError(sourceText.value.trim());
      if (requestId === requestSequence) errorTranslation.value = result;
    } else {
      const result = await translateText(sourceText.value.trim(), direction.value);
      if (requestId === requestSequence) translations.value = result;
    }
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
  await copyText(candidate.text, candidate.label, `original:${candidate.text}`);
}

async function copyErrorTranslation() {
  const result = errorTranslation.value;
  if (!result) return;
  const text = [
    `报错含义\n${result.meaning}`,
    `可能原因\n${result.possibleCauses.map((cause, index) => `${index + 1}. ${cause}`).join("\n")}`,
    `排查与解决方法\n${result.solutions.map((solution, index) => `${index + 1}. ${solution}`).join("\n")}`,
  ].join("\n\n");
  await copyText(text, "报错解读", "error-translation");
}

async function copyText(text: string, label: string, key: string) {
  const sequence = requestSequence;
  try {
    await navigator.clipboard.writeText(text);
    if (sequence !== requestSequence) return;
    error.value = "";
    copiedKey.value = key;
    copyMessage.value = `已复制：${label}`;
  } catch (cause) {
    if (sequence === requestSequence) error.value = `复制失败：${String(cause)}`;
  }
}

watch(sourceText, clearResult);
watch(errorMode, clearResult);
watch(() => props.open, (open) => {
  if (!open) {
    reset();
    errorMode.value = false;
    return;
  }
  void nextTick(() => document.querySelector<HTMLTextAreaElement>(".translation-source textarea")?.focus());
}, { immediate: true });
</script>

<template>
  <div v-if="open" class="modal-backdrop translation-backdrop" @click.self="close">
    <section class="panel translation-dialog" role="dialog" aria-modal="true" aria-labelledby="translation-dialog-title">
      <header>
        <div><h2 id="translation-dialog-title">中英翻译</h2><p>中英互译与报错解读，不保存翻译记录。</p></div>
        <button class="icon-button" title="关闭" aria-label="关闭翻译" @click="close">×</button>
      </header>

      <div class="translation-body">
        <label class="translation-mode">
          <span><b>报错翻译</b><small>{{ errorMode ? '中文解读 · 分析原因 · 给出解决方法' : '开启后可解读控制台或程序报错' }}</small></span>
          <input v-model="errorMode" class="translation-mode-switch" type="checkbox" role="switch" aria-label="报错翻译" :aria-checked="errorMode">
        </label>
        <div v-if="!errorMode" class="translation-direction" :class="{ manual:Boolean(manualDirection) }">
          <span><small>{{ manualDirection ? '手动方向' : sourceText.trim() ? '自动识别' : '等待输入' }}</small><b>{{ sourceLanguage }}</b></span>
          <button class="translation-swap" title="交换翻译方向" aria-label="交换翻译方向" @click="switchDirection">⇄</button>
          <span><small>目标语言</small><b>{{ targetLanguage }}</b></span>
          <button v-if="manualDirection" class="text-button" @click="restoreAutomaticDirection">恢复自动识别</button>
        </div>

        <label class="translation-field translation-source">
          <span><b>{{ errorMode ? '报错原文' : '原文' }}</b><small :class="{ over:characterCount > CHARACTER_LIMIT }">{{ characterCount }} / {{ CHARACTER_LIMIT }}</small></span>
          <textarea v-model="sourceText" rows="3" :placeholder="errorMode ? '粘贴控制台或程序报错信息，请先移除密钥等敏感内容…' : '输入需要翻译的中文或英文…'" @keydown="handleSourceKeydown"></textarea>
        </label>

        <section v-if="errorMode" class="translation-field translation-result">
          <span><b>报错解读</b><small>{{ loading ? '正在解读…' : errorTranslation ? '中文解读结果' : '等待翻译' }}</small></span>
          <div class="translation-results translation-error-results" :class="{ empty:!errorTranslation }" :aria-busy="loading">
            <p v-if="loading" class="translation-result-placeholder">DeepSeek 正在分析报错含义与解决方法…</p>
            <p v-else-if="!errorTranslation" class="translation-result-placeholder">翻译后查看报错含义、可能原因与解决步骤</p>
            <template v-else>
              <article class="translation-candidate translation-analysis-section">
                <h3>报错含义</h3>
                <p>{{ errorTranslation.meaning }}</p>
              </article>
              <article class="translation-candidate translation-analysis-section">
                <h3>可能原因</h3>
                <ol><li v-for="cause in errorTranslation.possibleCauses" :key="cause">{{ cause }}</li></ol>
              </article>
              <article class="translation-candidate translation-analysis-section">
                <h3>排查与解决方法</h3>
                <ol><li v-for="solution in errorTranslation.solutions" :key="solution">{{ solution }}</li></ol>
              </article>
              <p class="translation-copy-hint">原因分析仅供排查参考，请结合代码和运行环境确认。</p>
            </template>
          </div>
        </section>
        <section v-else class="translation-field translation-result">
          <span><b>译文候选</b><small>{{ loading ? '正在翻译…' : translations.length ? `${translations.length} 个${targetLanguage}结果` : '等待翻译' }}</small></span>
          <p v-if="displayCandidates.some(candidate => candidate.variants.length)" class="translation-copy-hint">点击命名文本即可复制 · 格式转换在本地完成</p>
          <div class="translation-results" :class="{ empty:!translations.length }">
            <p v-if="loading" class="translation-result-placeholder">DeepSeek 正在匹配不同语境的译法…</p>
            <p v-else-if="!translations.length" class="translation-result-placeholder">候选译文会显示在这里</p>
            <template v-else>
              <article v-for="(candidate,index) in displayCandidates" :key="`${candidate.label}:${candidate.text}`" class="translation-candidate">
                <header>
                  <span><i>{{ index + 1 }}</i><b>{{ candidate.label }}</b></span>
                  <button class="text-button translation-copy-original" @click="copyTranslation(candidate)">{{ copiedKey === `original:${candidate.text}` ? '已复制' : '复制原译文' }}</button>
                </header>
                <p class="translation-original">{{ candidate.text }}</p>
                <div v-if="candidate.variants.length" class="translation-naming-grid" role="group" :aria-label="`${candidate.label}的命名格式`">
                  <button v-for="variant in candidate.variants" :key="variant.id" type="button" class="translation-naming-option"
                    :class="{ copied:copiedKey === `${index}:${variant.id}` }"
                    :aria-label="`复制${variant.label}：${variant.text}`" :title="`点击复制 ${variant.text}`"
                    @click="copyText(variant.text, `${candidate.label} · ${variant.label}`, `${index}:${variant.id}`)">
                    <span><small>{{ variant.label }}</small><small class="translation-copy-state">{{ copiedKey === `${index}:${variant.id}` ? '已复制 ✓' : '点击复制' }}</small></span>
                    <code>{{ variant.text }}</code>
                  </button>
                </div>
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
        <span role="status" aria-live="polite">{{ copyMessage || 'Enter 翻译 · Shift + Enter 换行' }}</span>
        <div>
          <button class="button secondary" :disabled="loading || !sourceText" @click="reset">清空</button>
          <button v-if="errorMode" class="button secondary" :disabled="!errorTranslation" @click="copyErrorTranslation">复制解读</button>
          <button v-else class="button secondary" :disabled="!translations.length" @click="copyTranslation()">复制首选</button>
          <button class="button primary" :disabled="loading || !sourceText.trim()" @click="translate">{{ loading ? '翻译中…' : '翻译' }}</button>
        </div>
      </footer>
    </section>
  </div>
</template>
