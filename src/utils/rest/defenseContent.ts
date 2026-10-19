/** 塔防的轻量内容表；只保存标识和数值，不向存档写入素材。 */
export const DEFENSE_CORES = [
  { id: 'rapid', name: '机关塔芯', description: '双管速射，攻速增加30%，单发伤害降低8%。适合暴击与多重射击。', color: '#f4cc78', family: '速射', damage: .92, rate: 1.3, health: 0 },
  { id: 'cannon', name: '重炮塔芯', description: '单发伤害增加35%，攻速降低18%，自带基础溅射。适合爆破与燃烧。', color: '#f3aa79', family: '爆破', damage: 1.35, rate: .82, health: 0 },
  { id: 'tesla', name: '电磁塔芯', description: '伤害增加4%，自带一次电弧跳跃，护盾增加20点。适合冰霜与连锁闪电。', color: '#91dfe8', family: '元素', damage: 1.04, rate: 1, health: 0 },
  { id: 'bastion', name: '堡垒塔芯', description: '生命+60、每秒修复+0.8，自带反伤与持续伤害护环。护环、脉冲和震荡伤害提高20%。', color: '#b9daa0', family: '堡垒', damage: 1, rate: 1, health: 60 },
] as const;
export type DefenseCoreId = typeof DEFENSE_CORES[number]['id'] | 'classic';
// 与炮塔 SVG 的炮口位置共用；近身敌人的弹道不能从目标身后倒飞回来。
export function defenseBarrelLength(core:DefenseCoreId,tier:number){return core==='cannon'?54+tier*2:core==='rapid'?43+tier*2:core==='tesla'?20:38;}
export function defenseProjectileOrigin(core:DefenseCoreId,tier:number,target:{x:number;y:number}){
  const dx=target.x-300,dy=target.y-170,angle=Math.atan2(dy,dx);
  const length=Math.min(defenseBarrelLength(core,tier),Math.hypot(dx,dy)*.75);
  return{x:300+Math.cos(angle)*length,y:170+Math.sin(angle)*length,angle};
}
export const DEFENSE_FAMILIES = ['全部', '速射', '爆破', '元素', '堡垒', '成长', '进化'] as const;
export type DefensePreference = typeof DEFENSE_FAMILIES[number];
export const DEFENSE_EVOLUTIONS = [
  { id: 'firestorm', name: '连环火爆', family: '进化', requirements: { blast: 3, burn: 3, inferno: 1 }, description: '燃烧中的敌人死亡时，爆炸伤害周围敌人并传播火焰。升级继续增加伤害和半径。', color: '#ffac6b' },
  { id: 'froststorm', name: '碎冰风暴', family: '进化', requirements: { chain: 3, freeze: 2, storm: 1 }, description: '电弧击中冻结目标时触发碎冰溅射，升级继续增加伤害和半径。', color: '#9ce7f3' },
  { id: 'bulwark', name: '反应壁垒', family: '进化', requirements: { shield: 3, thorns: 3, barrier: 1 }, description: '护盾破裂时反击，伤害同时随攻击力与最大护盾成长。护盾恢复后可再次触发，升级增加伤害与半径并缩短冷却。', color: '#c5e5a3' },
  { id: 'volley', name: '穿云弹幕', family: '进化', requirements: { speed: 3, multi: 3, crit: 2 }, description: '每累计12轮有效射击，先释放金色扇形穿透齐射再普攻；塔上显示蓄能与释放提示。升级增加伤害和覆盖角度。', color: '#ffe8a0' },
] as const;
export type DefenseEvolutionId = typeof DEFENSE_EVOLUTIONS[number]['id'];
export type DefenseDamageKind = 'direct' | 'blast' | 'chain' | 'burn' | 'thorns' | 'nova' | 'evolution';
export const DEFENSE_DAMAGE_NAMES: Record<DefenseDamageKind, string> = { direct: '炮弹直击', blast: '爆破溅射', chain: '连锁电弧', burn: '持续灼烧', thorns: '反伤', nova: '范围技能', evolution: '组合进化' };
export const DEFENSE_BOSSES = {
  ram: { name: '破阵巨兽', description: '蓄力后冲锋，接近塔时造成更强打击。减速、护盾与单体火力都能应对。', color: '#efb37b' },
  bulwark: { name: '铁幕守卫', description: '周期恢复自身护盾，并保护附近敌人。穿甲和持续伤害更有效。', color: '#a6cce8' },
  summoner: { name: '巢母领主', description: '周期召唤蜂群。爆破、闪电与击杀回复适合处理援军。', color: '#d4a7de' },
} as const;
export type DefenseBossId = keyof typeof DEFENSE_BOSSES;
export const DEFENSE_REGIONS = [
  { id: 'forest', name: '林地前哨', description: '游荡者与蜂群交错，注意多方向来敌。' },
  { id: 'dusk', name: '暮色峡谷', description: '重甲和冲锋组合增多，兼顾穿甲与控制。' },
  { id: 'frost', name: '霜色高原', description: '远射者与支援怪协同，射程与群攻更重要。' },
  { id: 'ember', name: '余烬荒原', description: '多类精英混编，利用完整的技能组合推进。' },
] as const;
