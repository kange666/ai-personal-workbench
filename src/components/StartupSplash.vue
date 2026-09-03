<script setup lang="ts">
import appLogo from "../assets/app-logo.png";
import BrandWordmark from "./BrandWordmark.vue";
import { APP_BRAND } from "../utils/brand";

defineProps<{ slow?: boolean }>();
</script>

<template>
  <div class="startup-splash" role="status" aria-live="polite" :aria-label="`${APP_BRAND.name} 正在启动`">
    <div class="startup-aurora" aria-hidden="true"></div>
    <div class="startup-grid" aria-hidden="true"></div>
    <div class="startup-stars" aria-hidden="true">
      <i v-for="index in 18" :key="index"></i>
    </div>

    <section class="startup-stage">
      <div class="startup-map" aria-hidden="true">
        <svg viewBox="0 0 520 240" preserveAspectRatio="xMidYMid meet">
          <path class="map-line line-a" d="M30 188 128 137 205 170 260 120 350 74 490 42" />
          <path class="map-line line-b" d="M48 55 142 88 205 170 326 191 470 158" />
          <path class="map-line line-c" d="M142 88 260 120 326 191M260 120 470 158" />
          <circle cx="30" cy="188" r="4" />
          <circle cx="48" cy="55" r="3" />
          <circle cx="128" cy="137" r="4" />
          <circle cx="142" cy="88" r="5" />
          <circle cx="205" cy="170" r="3" />
          <circle cx="326" cy="191" r="4" />
          <circle cx="350" cy="74" r="5" />
          <circle cx="470" cy="158" r="4" />
          <circle cx="490" cy="42" r="3" />
        </svg>
        <div class="startup-scan"></div>
        <div class="startup-core"><span><img :src="appLogo" alt=""></span></div>
        <span class="startup-node node-one">本地数据</span>
        <span class="startup-node node-two">项目状态</span>
        <span class="startup-node node-three">工作记录</span>
        <span class="startup-node node-four">连接服务</span>
      </div>

      <div class="startup-copy">
        <small>PERSONAL WORKSPACE</small>
        <h1 class="startup-brand-heading" :aria-label="APP_BRAND.name"><BrandWordmark variant="splash" /></h1>
        <p>{{ slow ? "本地数据较多，仍在后台整理工作上下文…" : "正在连接你的项目、记录与工作状态…" }}</p>
        <div class="startup-progress" aria-hidden="true"><i></i></div>
        <div class="startup-steps" aria-hidden="true"><span>唤醒空间</span><span>读取上下文</span><span>同步状态</span></div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.startup-copy .startup-brand-heading{margin:16px 0 20px;font-size:inherit;letter-spacing:normal}
.startup-stage .startup-copy>small{color:var(--muted);font-size:9px;font-weight:500;letter-spacing:2.6px}
.startup-splash{position:fixed;inset:0;z-index:1000;overflow:hidden;display:grid;place-items:center;background:radial-gradient(circle at 50% 46%,color-mix(in srgb,var(--primary) 15%,transparent),transparent 28%),linear-gradient(145deg,color-mix(in srgb,var(--bg) 96%,#060817),var(--bg));color:var(--text);isolation:isolate}
.startup-aurora{position:absolute;width:min(82vw,1050px);height:min(54vw,670px);border-radius:50%;background:conic-gradient(from 210deg,transparent,color-mix(in srgb,var(--primary) 14%,transparent),transparent 38%,color-mix(in srgb,var(--success) 8%,transparent),transparent 72%);filter:blur(54px);animation:startup-aurora 9s linear infinite}
.startup-grid{position:absolute;inset:-30%;background-image:linear-gradient(color-mix(in srgb,var(--primary) 8%,transparent) 1px,transparent 1px),linear-gradient(90deg,color-mix(in srgb,var(--primary) 8%,transparent) 1px,transparent 1px);background-size:46px 46px;mask-image:radial-gradient(circle at center,#000 0,transparent 63%);transform:perspective(650px) rotateX(62deg) translateY(32%);animation:startup-grid-drift 5s linear infinite}
.startup-stars i{position:absolute;width:2px;height:2px;border-radius:50%;background:color-mix(in srgb,var(--primary) 76%,#fff);box-shadow:0 0 8px currentColor;opacity:.58;animation:startup-star 2.6s ease-in-out infinite}.startup-stars i:nth-child(1){left:7%;top:18%}.startup-stars i:nth-child(2){left:14%;top:72%;animation-delay:-.8s}.startup-stars i:nth-child(3){left:22%;top:30%;animation-delay:-1.7s}.startup-stars i:nth-child(4){left:31%;top:82%;animation-delay:-.4s}.startup-stars i:nth-child(5){left:42%;top:14%;animation-delay:-2s}.startup-stars i:nth-child(6){left:53%;top:9%;animation-delay:-1.1s}.startup-stars i:nth-child(7){left:63%;top:22%;animation-delay:-.3s}.startup-stars i:nth-child(8){left:72%;top:76%;animation-delay:-1.5s}.startup-stars i:nth-child(9){left:86%;top:29%;animation-delay:-2.2s}.startup-stars i:nth-child(10){left:93%;top:64%;animation-delay:-.7s}.startup-stars i:nth-child(11){left:5%;top:48%;animation-delay:-1.3s}.startup-stars i:nth-child(12){left:18%;top:91%;animation-delay:-2.4s}.startup-stars i:nth-child(13){left:39%;top:66%;animation-delay:-.9s}.startup-stars i:nth-child(14){left:58%;top:86%;animation-delay:-1.8s}.startup-stars i:nth-child(15){left:69%;top:42%;animation-delay:-.2s}.startup-stars i:nth-child(16){left:79%;top:10%;animation-delay:-1.2s}.startup-stars i:nth-child(17){left:88%;top:88%;animation-delay:-2.1s}.startup-stars i:nth-child(18){left:97%;top:18%;animation-delay:-.6s}
.startup-stage{position:relative;width:min(680px,calc(100vw - 64px));display:grid;justify-items:center;gap:8px}
.startup-map{position:relative;width:min(620px,88vw);height:286px;filter:drop-shadow(0 18px 54px rgba(0,0,0,.28))}.startup-map svg{position:absolute;inset:12px 0 0;width:100%;height:240px;overflow:visible}.startup-map circle{fill:var(--primary);stroke:color-mix(in srgb,var(--primary) 25%,#fff);stroke-width:2;transform-box:fill-box;transform-origin:center;animation:startup-node-pulse 2.2s ease-in-out infinite}.startup-map circle:nth-of-type(2n){animation-delay:-1.1s}.map-line{fill:none;stroke:color-mix(in srgb,var(--primary) 42%,transparent);stroke-width:1;stroke-linecap:round;stroke-dasharray:5 10}.line-a{animation:startup-line 5s linear infinite}.line-b{animation:startup-line 6.5s linear infinite reverse}.line-c{stroke:color-mix(in srgb,var(--success) 34%,transparent);animation:startup-line 4.2s linear infinite}
.startup-scan{position:absolute;left:8%;right:8%;top:16px;height:1px;background:linear-gradient(90deg,transparent,var(--primary),transparent);box-shadow:0 0 14px color-mix(in srgb,var(--primary) 62%,transparent);opacity:0;animation:startup-scan 3.4s ease-in-out infinite}
.startup-core{position:absolute;left:50%;top:132px;width:104px;height:104px;transform:translate(-50%,-50%) rotate(45deg);border:1px solid color-mix(in srgb,var(--primary) 48%,transparent);background:color-mix(in srgb,var(--surface) 88%,transparent);box-shadow:0 0 0 12px color-mix(in srgb,var(--primary) 5%,transparent),0 0 58px color-mix(in srgb,var(--primary) 30%,transparent);animation:startup-core-frame 4s ease-in-out infinite}.startup-core::before,.startup-core::after{content:"";position:absolute;inset:9px;border:1px solid color-mix(in srgb,var(--primary) 18%,transparent)}.startup-core::after{inset:-18px;border-style:dashed;animation:startup-core-orbit 9s linear infinite}.startup-core>span{position:absolute;inset:17px;display:grid;place-items:center;border-radius:15px;background:linear-gradient(145deg,color-mix(in srgb,var(--primary) 78%,#fff),var(--primary-strong));transform:rotate(-45deg);box-shadow:inset 0 0 18px rgba(255,255,255,.16)}.startup-core img{width:48px;height:48px;object-fit:contain;filter:drop-shadow(0 5px 10px rgba(0,0,0,.28))}
.startup-node{position:absolute;min-width:74px;padding:5px 8px;border:1px solid color-mix(in srgb,var(--primary) 28%,var(--line));border-radius:5px;background:color-mix(in srgb,var(--surface) 84%,transparent);color:var(--muted);font-size:9px;letter-spacing:.6px;text-align:center;backdrop-filter:blur(8px);animation:startup-label 2.8s ease-in-out infinite}.node-one{left:8%;top:189px}.node-two{left:19%;top:61px;animation-delay:-.7s}.node-three{right:17%;top:47px;animation-delay:-1.4s}.node-four{right:5%;top:169px;animation-delay:-2.1s}
.startup-copy{width:min(430px,82vw);display:grid;justify-items:center;text-align:center}.startup-copy>small{color:var(--primary);font-size:10px;font-weight:800;letter-spacing:3.2px}.startup-copy h1{margin:9px 0 6px;font-size:30px;letter-spacing:5px}.startup-copy p{min-height:19px;margin:0;color:var(--muted);font-size:11px}.startup-progress{width:100%;height:3px;margin-top:22px;overflow:hidden;border-radius:3px;background:color-mix(in srgb,var(--primary) 11%,var(--surface-2));box-shadow:inset 0 0 0 1px color-mix(in srgb,var(--primary) 11%,transparent)}.startup-progress i{display:block;width:38%;height:100%;border-radius:inherit;background:linear-gradient(90deg,transparent,var(--primary),color-mix(in srgb,var(--success) 78%,var(--primary)),transparent);filter:drop-shadow(0 0 5px var(--primary));animation:startup-progress 1.8s ease-in-out infinite}.startup-steps{width:100%;margin-top:9px;display:flex;justify-content:space-between;color:color-mix(in srgb,var(--muted) 72%,transparent);font-size:8px;letter-spacing:.8px}.startup-steps span{animation:startup-step 3s ease-in-out infinite}.startup-steps span:nth-child(2){animation-delay:.65s}.startup-steps span:nth-child(3){animation-delay:1.3s}
.startup-screen-leave-active{transition:opacity .38s ease,filter .38s ease}.startup-screen-leave-to{opacity:0;filter:blur(8px)}
@keyframes startup-aurora{to{transform:rotate(360deg)}}@keyframes startup-grid-drift{to{background-position:0 46px,46px 0}}@keyframes startup-star{50%{opacity:1;transform:scale(2)}}@keyframes startup-line{to{stroke-dashoffset:-120}}@keyframes startup-node-pulse{50%{transform:scale(1.65);opacity:.55}}@keyframes startup-scan{0%{top:22px;opacity:0}12%{opacity:.8}82%{opacity:.5}100%{top:244px;opacity:0}}@keyframes startup-core-frame{50%{box-shadow:0 0 0 16px color-mix(in srgb,var(--primary) 4%,transparent),0 0 74px color-mix(in srgb,var(--primary) 35%,transparent)}}@keyframes startup-core-orbit{to{transform:rotate(360deg)}}@keyframes startup-label{50%{border-color:color-mix(in srgb,var(--primary) 54%,var(--line));color:var(--text)}}@keyframes startup-progress{0%{transform:translateX(-110%)}100%{transform:translateX(365%)}}@keyframes startup-step{0%,100%{color:color-mix(in srgb,var(--muted) 55%,transparent)}35%{color:var(--primary)}}
@media(max-height:760px){.startup-map{height:238px;transform:scale(.82);margin:-22px 0}.startup-copy h1{font-size:26px}.startup-progress{margin-top:15px}}
@media(prefers-reduced-motion:reduce){.startup-splash *{animation:none!important}.startup-progress i{width:100%;opacity:.58}.startup-stars i{opacity:.65}}
</style>
