import lunarJs from "lunar-javascript";

export interface AlmanacDetail {
  date: string;
  week: string;
  lunarDate: string;
  ganZhi: string;
  zodiac: string;
  jieQi: string;
  festivals: string[];
  yi: string[];
  ji: string[];
  auspiciousGods: string[];
  inauspiciousGods: string[];
  clash: string;
  sha: string;
  duty: string;
  heavenlyGod: string;
  luck: string;
  mansion: string;
  mansionLuck: string;
  joyPosition: string;
  fortunePosition: string;
  wealthPosition: string;
  pengZu: string[];
}

export function getAlmanac(dateText: string): AlmanacDetail {
  const [year, month, day] = dateText.split("-").map(Number);
  const solar = lunarJs.Solar.fromYmd(year, month, day);
  const lunar = solar.getLunar();
  return {
    date: dateText,
    week: `星期${solar.getWeekInChinese()}`,
    lunarDate: `农历${lunar.getYearInChinese()}年${lunar.getMonthInChinese()}月${lunar.getDayInChinese()}`,
    ganZhi: `${lunar.getYearInGanZhi()}年 ${lunar.getMonthInGanZhi()}月 ${lunar.getDayInGanZhi()}日`,
    zodiac: `${lunar.getYearShengXiao()}年 · ${lunar.getDayShengXiao()}日`,
    jieQi: lunar.getJieQi() || "非节气日",
    festivals: [...solar.getFestivals(), ...lunar.getFestivals()],
    yi: lunar.getDayYi(),
    ji: lunar.getDayJi(),
    auspiciousGods: lunar.getDayJiShen(),
    inauspiciousGods: lunar.getDayXiongSha(),
    clash: `冲${lunar.getDayChongDesc()}`,
    sha: `煞${lunar.getDaySha()}`,
    duty: `${lunar.getZhiXing()}日`,
    heavenlyGod: lunar.getDayTianShen(),
    luck: lunar.getDayTianShenLuck(),
    mansion: lunar.getXiu(),
    mansionLuck: lunar.getXiuLuck(),
    joyPosition: lunar.getDayPositionXiDesc(),
    fortunePosition: lunar.getDayPositionFuDesc(),
    wealthPosition: lunar.getDayPositionCaiDesc(),
    pengZu: [lunar.getPengZuGan(), lunar.getPengZuZhi()],
  };
}

export function lunarDayLabel(dateText: string): string {
  const [year, month, day] = dateText.split("-").map(Number);
  const lunar = lunarJs.Solar.fromYmd(year, month, day).getLunar();
  const jieQi = lunar.getJieQi();
  if (jieQi) return jieQi;
  if (lunar.getDay() === 1) return `${lunar.getMonthInChinese()}月`;
  return lunar.getDayInChinese();
}
