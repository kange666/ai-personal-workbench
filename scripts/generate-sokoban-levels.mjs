// 离线反向推演：从箱子全部归位开始拉箱，每次操作都记录可逆的通关路线。
// 生成文件纳入源码，运行游戏时不依赖随机生成或网络。
import { writeFileSync } from 'node:fs';
let seed = 81219;
const random = () => ((seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 4294967296);
const directions = [{ x: 0, y: -1, c: 'U', inverse: 'D' }, { x: 1, y: 0, c: 'R', inverse: 'L' }, { x: 0, y: 1, c: 'D', inverse: 'U' }, { x: -1, y: 0, c: 'L', inverse: 'R' }];
const levels = [];
const seen = new Set();
for (let attempt = 0; levels.length < 74 && attempt < 20000; attempt++) {
  const index = levels.length + 6, chapter = Math.floor(index / 10);
  const width = chapter < 3 ? 7 : chapter < 6 ? 8 : 9, height = chapter < 5 ? 7 : 8;
  const floors = new Set();
  for (let y = 1; y < height - 1; y++) for (let x = 1; x < width - 1; x++) if (random() > .10 + chapter * .008) floors.add(y * width + x);
  const inner = [...floors].filter(p => directions.every(d => floors.has(p + d.x + d.y * width)));
  const count = chapter < 2 ? 2 : chapter < 5 ? 3 : 4;
  if (inner.length < count + 1) continue;
  inner.sort(() => random() - .5);
  const goals = inner.slice(0, count), boxes = [...goals];
  let player = inner[count], reverse = [], pulls = 0;
  const reachable = () => {
    const paths = new Map([[player, '']]), queue = [player];
    for (let i = 0; i < queue.length; i++) for (const d of directions) {
      const next = queue[i] + d.x + d.y * width;
      if (floors.has(next) && !boxes.includes(next) && !paths.has(next)) { paths.set(next, paths.get(queue[i]) + d.c); queue.push(next); }
    }
    return paths;
  };
  let previous = -1;
  for (let step = 0; step < 16 + chapter * 7; step++) {
    const paths = reachable(), options = [];
    boxes.forEach((box, b) => directions.forEach(d => {
      const next = box + d.x + d.y * width, stand = next + d.x + d.y * width;
      if (next !== previous && paths.has(next) && floors.has(stand) && !boxes.includes(stand)) options.push({ b, d, next, stand, path: paths.get(next) });
    }));
    if (!options.length) break;
    const option = options[Math.floor(random() * options.length)];
    reverse.push(...option.path, option.d.c); previous = boxes[option.b]; boxes[option.b] = option.next; player = option.stand; pulls++;
  }
  if (pulls < 7 + chapter * 2 || boxes.every(b => goals.includes(b))) continue;
  const map = Array.from({ length: height }, (_, y) => Array.from({ length: width }, (_, x) => {
    const p = y * width + x;
    return !floors.has(p) ? '#' : p === player ? (goals.includes(p) ? '+' : '@') : boxes.includes(p) ? (goals.includes(p) ? '*' : '$') : goals.includes(p) ? '.' : ' ';
  }).join(''));
  const key = map.join('\n'); if (seen.has(key)) continue;
  // 验证反演路线，并截断在首次全部归位处，避免已通关后仍包含多余行走。
  const solution = reverse.reverse().map(c => directions.find(d => d.c === c).inverse).join('');
  let hero = player, testBoxes = [...boxes], verified = '';
  for (const c of solution) {
    const d = directions.find(d => d.c === c), next = hero + d.x + d.y * width, beyond = next + d.x + d.y * width;
    if (!floors.has(next) || (testBoxes.includes(next) && (!floors.has(beyond) || testBoxes.includes(beyond)))) throw new Error('反演路线无效');
    if (testBoxes.includes(next)) testBoxes[testBoxes.indexOf(next)] = beyond;
    hero = next; verified += c;
    if (testBoxes.every(b => goals.includes(b))) break;
  }
  if (!testBoxes.every(b => goals.includes(b))) throw new Error('未解出');
  seen.add(key); levels.push({ name: `${['入门货舱','转角调度','双箱协作','窄巷回转','三箱编组','仓库穿梭','四箱调度','终章货场'][chapter]} ${index % 10 + 1}`, chapter: `第 ${chapter + 1} 章`, map, solution: verified });
}
if (levels.length !== 74) throw new Error(`只生成 ${levels.length} 关`);
writeFileSync(new URL('../src/utils/rest/sokobanLevels.ts', import.meta.url), `// 由 scripts/generate-sokoban-levels.mjs 生成；solution 为逐步验证的通关证书。\nexport const EXTRA_SOKOBAN_LEVELS = ${JSON.stringify(levels, null, 2)};\n`, 'utf8');
console.log(`已生成并逐步验证 ${levels.length} 个不同关卡`);
