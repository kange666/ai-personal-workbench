export const restGames = [
  { id: "wiring", name: "接线解谜", hint: "点亮整座空间站", color: "mint" },
  { id: "breakout", name: "打砖块", hint: "接住下一次反弹", color: "blue" },
  { id: "tower", name: "堆塔挑战", hint: "向星空再高一层", color: "violet" },
  { id: "mota", name: "魔塔", hint: "经典 21 层冒险", color: "teal" },
  { id: "sokoban", name: "推箱子", hint: "给每只箱子找个家", color: "amber" },
  { id: "gomoku", name: "五子棋", hint: "与电脑安静下一局", color: "rose" },
] as const;
export type RestGameId = typeof restGames[number]["id"];
