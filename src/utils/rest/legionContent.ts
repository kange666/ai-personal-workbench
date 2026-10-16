export const LEGION_UNITS = [
  { id: 'guard', name: '盾卫', role: '前排承伤', unlock: 1, hp: 170, damage: 9, rate: 1, range: 40, speed: 42, armor: .3, cost: 20, color: '#81c7e8', detail: '优先迎战最近敌人；基础减伤30%，坚守强化可保护相邻友军。' },
  { id: 'sword', name: '剑士', role: '近战连斩', unlock: 1, hp: 115, damage: 16, rate: 1.3, range: 38, speed: 56, armor: .08, cost: 16, color: '#c6dfe9', detail: '快速追击前排；连斩和横扫强化适合处理成群敌人。' },
  { id: 'archer', name: '弓手', role: '远程穿透', unlock: 1, hp: 66, damage: 16, rate: 1.1, range: 250, speed: 38, armor: 0, cost: 18, color: '#edc17a', detail: '保持距离持续射击；双发、穿透与爆裂箭可组成箭雨流派。' },
  { id: 'mage', name: '法师', role: '元素清场', unlock: 3, hp: 65, damage: 23, rate: .8, range: 230, speed: 37, armor: 0, cost: 24, color: '#a7b6ff', detail: '基础火球可伤及邻近敌人；冰霜、燃烧与闪电会改变施法效果。' },
  { id: 'priest', name: '祭司', role: '群体治疗', unlock: 5, hp: 82, damage: 12, rate: .8, range: 220, speed: 36, armor: .05, cost: 26, color: '#8ee0b3', detail: '优先治疗血量比例最低的友军；全员健康时使用圣光攻击。' },
  { id: 'giant', name: '巨人', role: '重装突破', unlock: 8, hp: 320, damage: 38, rate: .6, range: 55, speed: 33, armor: .24, cost: 40, color: '#94b9b1', detail: '重击同时伤害近旁敌人；践踏可扩大范围并短暂减速。' },
] as const;
export type LegionUnitKind = typeof LEGION_UNITS[number]['id'];
export type LegionStrategy = 'balanced' | 'arrows' | 'elements' | 'iron';
export const LEGION_STRATEGIES = [
  { id: 'balanced', name: '均衡推进', detail: '兼顾输出、前排和治疗，适合全自动挂机。' },
  { id: 'arrows', name: '箭雨军团', detail: '优先弓手、双发、穿透和爆裂箭；保留盾卫与治疗。' },
  { id: 'elements', name: '雷火军团', detail: '优先法师、连锁、燃烧与冰霜；元素联动清理敌群。' },
  { id: 'iron', name: '钢铁军团', detail: '优先盾卫、巨人、治疗与反击，稳步积累战力。' },
] as const;
export const LEGION_CARDS = [
  { id: 'power', name: '战旗鼓舞', family: 'balanced', detail: '全体攻击与治疗提高12%。' },
  { id: 'haste', name: '战斗节奏', family: 'balanced', detail: '全体攻击与治疗频率提高10%。' },
  { id: 'vitality', name: '生命祝福', family: 'iron', detail: '全体最大生命提高15%，立即恢复15%生命。' },
  { id: 'armor', name: '精制护甲', family: 'iron', detail: '护甲系数增加0.12，伤害按1÷(1+护甲系数)结算。' },
  { id: 'double', name: '双发箭', family: 'arrows', detail: '弓手额外射出一支箭，造成基础攻击45%的伤害。' },
  { id: 'pierce', name: '穿透箭', family: 'arrows', detail: '箭矢穿透一个邻近敌人，造成55%伤害。' },
  { id: 'explosive', name: '爆裂箭', family: 'arrows', detail: '箭矢命中后爆炸，对75范围敌人造成35%伤害。' },
  { id: 'precision', name: '鹰眼', family: 'arrows', detail: '弓手攻击提高18%，射程增加12。' },
  { id: 'volley', name: '齐射号令', family: 'arrows', detail: '每第四次射击追加60%伤害的齐射。' },
  { id: 'focus', name: '专注猎杀', family: 'arrows', detail: '弓手对精英、首领和重甲单位伤害提高25%。' },
  { id: 'chain', name: '连锁闪电', family: 'elements', detail: '法师闪电额外跳跃一个邻近敌人，造成55%伤害。' },
  { id: 'burn', name: '烈焰余烬', family: 'elements', detail: '法师附加持续3秒的燃烧，每秒造成攻击力12%的伤害。' },
  { id: 'frost', name: '霜冻法术', family: 'elements', detail: '法师命中使敌人移动减慢30%，持续2秒。' },
  { id: 'blast', name: '火球扩散', family: 'elements', detail: '法师火球范围增加15，溅射伤害提高20%。' },
  { id: 'shatter', name: '冰雷碎裂', family: 'elements', detail: '被减速的敌人受到法师攻击时额外承受30%伤害。' },
  { id: 'inferno', name: '余烬传染', family: 'elements', detail: '燃烧中的敌人倒下时，对80范围敌人造成法师攻击力30%的伤害。' },
  { id: 'heal', name: '丰沛圣光', family: 'iron', detail: '祭司治疗提高25%，同时治疗另一名受伤友军。' },
  { id: 'thorns', name: '盾墙反击', family: 'iron', detail: '盾卫受到攻击后向攻击者反击所受伤害的35%。' },
  { id: 'bulwark', name: '坚守阵线', family: 'iron', detail: '盾卫附近100范围友军护甲系数增加0.087；无护甲时约减伤8%，多个盾卫光环不叠加。' },
  { id: 'stomp', name: '震地践踏', family: 'iron', detail: '巨人范围增加18，重击使敌人减速30%，持续2秒。' },
  { id: 'cleave', name: '横扫连斩', family: 'balanced', detail: '剑士伤及55范围其他敌人，造成40%攻击伤害。' },
  { id: 'recruit', name: '召唤共鸣', family: 'balanced', detail: '新增编队的召唤等待缩短12%，新兵入场恢复友军少量生命；本关阵亡名额不重复召唤。' },
  { id: 'bounty', name: '远征补给', family: 'balanced', detail: '击杀金币和召唤精华提高15%，持续增加自动训练效率。' },
  { id: 'resolve', name: '绝境斗志', family: 'balanced', detail: '生命低于50%的友军攻击和治疗提高20%。' },
] as const;
export type LegionCardId = typeof LEGION_CARDS[number]['id'];
export const LEGION_REGIONS = [
  { name: '森林边境', sky: '#60889a', floor: '#536e4d', accent: '#96c282' },
  { name: '暮色荒原', sky: '#846b82', floor: '#766444', accent: '#e1b572' },
  { name: '寒霜遗迹', sky: '#768fae', floor: '#728c91', accent: '#b3d9e8' },
  { name: '熔火裂谷', sky: '#915d5b', floor: '#695552', accent: '#e7ac79' },
] as const;
export type LegionEnemyKind = 'goblin' | 'runner' | 'skeleton' | 'armored' | 'shaman' | 'swarm' | 'warlord' | 'behemoth' | 'necromancer';
export const LEGION_ENEMIES: Record<LegionEnemyKind, { name: string; hp: number; damage: number; range: number; speed: number; rate: number; armor: number }> = {
  goblin: { name: '哥布林', hp: 55, damage: 8, range: 35, speed: 42, rate: .9, armor: 0 },
  runner: { name: '疾行狼', hp: 40, damage: 9, range: 34, speed: 70, rate: 1.3, armor: 0 },
  skeleton: { name: '骷髅弓手', hp: 42, damage: 8, range: 210, speed: 30, rate: .8, armor: 0 },
  armored: { name: '重甲卫士', hp: 110, damage: 13, range: 42, speed: 27, rate: .65, armor: .5 },
  shaman: { name: '邪术萨满', hp: 58, damage: 11, range: 200, speed: 29, rate: .7, armor: 0 },
  swarm: { name: '洞穴小怪', hp: 26, damage: 6, range: 30, speed: 57, rate: 1.1, armor: 0 },
  warlord: { name: '蛮荒督军', hp: 740, damage: 28, range: 60, speed: 26, rate: .65, armor: .16 },
  behemoth: { name: '装甲巨兽', hp: 1000, damage: 38, range: 65, speed: 23, rate: .5, armor: .65 },
  necromancer: { name: '亡灵祭司', hp: 650, damage: 22, range: 225, speed: 27, rate: .7, armor: .1 },
};
