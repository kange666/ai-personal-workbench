<template>
  <!-- 静态图形定义一次；场上的同类敌人通过 use 复用，不加载图片或逐帧素材。 -->
  <defs>
    <g id="defense-eyes" fill="#f7e5cd"><ellipse cx="-4" cy="-3" rx="2" ry="3"/><ellipse cx="4" cy="-3" rx="2" ry="3"/></g>
    <g id="defense-normal">
      <ellipse cy="9" rx="13" ry="3" fill="#1d3541" opacity=".16"/>
      <path d="M-12 4Q-14-11-2-14 11-16 13-2L11 7Q6 11 0 8-8 12-12 4Z" fill="#7963a5" stroke="#433956" stroke-width="1.5"/>
      <path d="M-9-4Q-8-10-3-10" fill="none" stroke="#a590c4" stroke-width="2" stroke-linecap="round"/>
      <use href="#defense-eyes"/><path d="M-8 6-6 10M7 6 9 10" stroke="#514068" stroke-width="3" stroke-linecap="round"/>
    </g>
    <g id="defense-swarm">
      <path d="M-4-3Q-10-14-21-11L-15-5-16 2-8 0-5 5M4-3Q10-14 21-11L15-5 16 2 8 0 5 5" fill="#745392" stroke="#443650" stroke-width="1.3"/>
      <path d="M-6-9-6 5Q0 13 6 5V-9L1-5Z" fill="#463958"/><use href="#defense-eyes" transform="scale(.8)"/>
    </g>
    <g id="defense-runner">
      <path d="M-11 0-17-6M-10 4-18 5" stroke="#c5b0a3" stroke-width="1.5" stroke-linecap="round"/>
      <path d="M-7-10 3-15 13-5 8 5-5 8-10 2Z" fill="#c58159" stroke="#664936" stroke-width="1.5"/>
      <path d="m-4 5-6 6m16-7 7 6" stroke="#664936" stroke-width="3" stroke-linecap="round"/><use href="#defense-eyes"/>
    </g>
    <g id="defense-armored">
      <path d="m-10 4-6 6m7-4-2 7m20-9 6 6m-6-4 2 7" stroke="#574a43" stroke-width="3" stroke-linecap="round"/>
      <path d="M-15-2-12-10-3-15 9-12 15-3 11 8-7 9Z" fill="#99724b" stroke="#5b4940" stroke-width="1.8"/>
      <path d="m-12-6 9 2 7-9m-7 9 4 11m-4-11 15 2" fill="none" stroke="#c39761" stroke-width="1.7"/>
      <path d="m-10-10-4-6 7 2m13 0 5-6 1 8" fill="#c0a174" stroke="#5b4940"/>
      <use href="#defense-eyes" transform="translate(0 4)"/>
    </g>
    <g id="defense-splitter">
      <use href="#defense-normal"/><path d="M-1-13-4-5 2-1-1 7" fill="none" stroke="#d3b6f2" stroke-width="2"/>
    </g>
    <g id="defense-healer">
      <path d="M-10 8Q-16-1-8-12L0-16 10-9 12 4 6 10Z" fill="#6da186" stroke="#38635a" stroke-width="1.5"/>
      <path d="M-5-6H5M0-11V-1" stroke="#d9f1bb" stroke-width="3"/><use href="#defense-eyes" transform="translate(0 8) scale(.8)"/>
    </g>
    <g id="defense-shooter">
      <path d="M-12 7-9-11 0-17 9-11 12 7 4 10-3 7Z" fill="#8579a3" stroke="#443b5a" stroke-width="1.5"/>
      <use href="#defense-eyes"/><path d="M7-1 16-8 18-6 10 3" fill="#ccb89a" stroke="#594e50"/>
    </g>
    <g id="defense-shielded">
      <path d="M-12 6-10-10-4-16 7-12 11 6 3 10Z" fill="#88b6c4" stroke="#476575" stroke-width="1.5"/>
      <use href="#defense-eyes"/><path d="m10-7 7-4 6 4-1 13-6 5-6-5Z" fill="#527f9b" stroke="#c1ebeb" stroke-width="1.5"/>
      <path d="M16-5V5m-3-5h6" stroke="#c1ebeb" stroke-width="1.5"/>
    </g>
    <g id="defense-boss">
      <ellipse cy="18" rx="26" ry="5" fill="#1c333b" opacity=".2"/>
      <path d="M-13 5-17 17-7 19-3 9M5 8 10 18 21 16 15 3" fill="#505b68" stroke="#323e4b" stroke-width="2"/>
      <path d="M-18-15-27-10-29 2-22 8-16 0M17-15 28-7 30 5 23 10 16 0" fill="#657380" stroke="#323e4b" stroke-width="2"/>
      <path d="M-17-15-10-24 9-24 18-13 14 6 0 13-15 4Z" fill="#657380" stroke="#323e4b" stroke-width="2"/>
      <path d="m-12-20-8-11 1 13 8 5M11-20l8-11-1 14-7 4" fill="#d5ad68" stroke="#7b6849" stroke-width="1.5"/>
      <path d="m-11-8 10 4 13-6M-1-4 2 8" fill="none" stroke="#efb45c" stroke-width="2.5"/>
      <path d="m-9-13 6 2m6 0 6-2" stroke="#ffe1a1" stroke-width="2.5" stroke-linecap="round"/>
    </g>
    <g id="defense-pine"><path d="M0-28-14-7-8-8-19 6H-5V15H5V6H19L8-8 14-7Z" fill="currentColor"/></g>
  </defs>
</template>
