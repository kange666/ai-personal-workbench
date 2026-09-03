<script setup lang="ts">
import { APP_BRAND } from "../utils/brand";

withDefaults(defineProps<{ variant?: "compact" | "hero" | "splash" }>(), {
  variant: "compact",
});
</script>

<template>
  <span class="brand-wordmark" :class="`brand-wordmark--${variant}`">
    <span class="brand-wordmark__chinese">{{ APP_BRAND.chinese }}</span>
    <span class="brand-wordmark__english" lang="en">{{ APP_BRAND.english }}</span>
  </span>
</template>

<style scoped>
.brand-wordmark {
  display: inline-flex;
  flex: 0 0 auto;
  flex-direction: column;
  align-items: flex-start;
  gap: 5px;
  color: inherit;
  white-space: nowrap;
}
.brand-wordmark__chinese {
  font-family: "Microsoft YaHei UI", "Microsoft YaHei", sans-serif;
  font-size: 19px;
  font-weight: 600;
  line-height: 1.15;
  letter-spacing: .18em;
  margin-right: -.18em;
}
.brand-wordmark__english {
  font-family: "Segoe UI Variable Display", "Segoe UI", sans-serif;
  font-size: 9px;
  font-weight: 600;
  line-height: 1;
  letter-spacing: .22em;
  margin-right: -.22em;
  color: var(--muted);
}
.brand-wordmark--hero {
  flex-direction: row;
  align-items: center;
  gap: 20px;
}
.brand-wordmark--hero .brand-wordmark__chinese { font-size: 32px; }
.brand-wordmark--hero .brand-wordmark__english {
  padding: 4px 0 4px 20px;
  border-left: 1px solid var(--line);
  font-size: 13px;
  letter-spacing: .3em;
}
.brand-wordmark--splash { align-items: center; gap: 15px; }
.brand-wordmark--splash .brand-wordmark__chinese { font-size: 44px; letter-spacing: .24em; margin-right: -.24em; }
.brand-wordmark--splash .brand-wordmark__english { font-size: 12px; letter-spacing: .46em; margin-right: -.46em; color: var(--primary); }
@media (max-width: 1280px) {
  .brand-wordmark--hero { gap: 15px; }
  .brand-wordmark--hero .brand-wordmark__chinese { font-size: 29px; }
  .brand-wordmark--hero .brand-wordmark__english { padding-left: 15px; font-size: 11px; }
}
@media (max-height: 760px) {
  .brand-wordmark--splash .brand-wordmark__chinese { font-size: 38px; }
}
</style>
