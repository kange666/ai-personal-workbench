type Schema = Record<string, unknown>;
const object = (value: unknown): Schema => value && typeof value === "object" && !Array.isArray(value) ? value as Schema : {};

// 只识别明确的分页字段，不根据普通业务字段名猜测分页含义。
export function paginationDefault(name: string): number | undefined {
  const key = name.replace(/[-_]/g, "").toLowerCase();
  if (["page", "pagenum", "pagenumber", "pageindex", "current", "currentpage"].includes(key)) return 1;
  if (["pagesize", "perpage", "limit", "pagelimit"].includes(key)) return 10;
  if (key === "offset") return 0;
  return undefined;
}

export function apiTestDefault(schema: Schema, name: string, depth = 0): unknown {
  if (depth > 5) return null;
  // 文档提供的值优先，保留合法的 0 和 false。
  for (const key of ["default", "example", "const"]) {
    if (schema[key] !== undefined && schema[key] !== null && schema[key] !== "") return schema[key];
  }
  if (Array.isArray(schema.enum)) {
    const value = schema.enum.find(item => item !== null && item !== "");
    if (value !== undefined) return value;
  }
  const page = paginationDefault(name);
  if (page !== undefined) return page;
  for (const key of ["oneOf", "anyOf"]) {
    if (Array.isArray(schema[key]) && schema[key].length) return apiTestDefault(object(schema[key][0]), name, depth + 1);
  }
  const type = schema.type ?? (schema.properties ? "object" : schema.items ? "array" : "string");
  if (type === "object") {
    const properties = object(schema.properties);
    return Object.fromEntries((Array.isArray(schema.required) ? schema.required : [])
      .filter((key): key is string => typeof key === "string" && key in properties && object(properties[key]).readOnly !== true)
      .map(key => [key, apiTestDefault(object(properties[key]), key, depth + 1)]));
  }
  if (type === "array") return Array.from({length: Math.min(Math.max(Number(schema.minItems) || 0, 0), 5)}, () => apiTestDefault(object(schema.items), name, depth + 1));
  if (type === "boolean") return true;
  if (type === "integer" || type === "number") {
    let value = typeof schema.minimum === "number" ? schema.minimum : 1;
    if (typeof schema.exclusiveMinimum === "number") value = schema.exclusiveMinimum + 1;
    if (typeof schema.maximum === "number") value = Math.min(value, schema.maximum);
    return type === "integer" ? Math.ceil(value) : value;
  }
  const key = name.toLowerCase();
  if (schema.format === "date") return new Date().toISOString().slice(0, 10);
  if (schema.format === "date-time") return new Date().toISOString();
  if (schema.format === "uuid") return "00000000-0000-4000-8000-000000000001";
  if (schema.format === "email" || key.includes("email")) return "test@example.com";
  if (/phone|mobile|tel/.test(key)) return "13800000000";
  if (key === "id" || key.endsWith("id")) return "1";
  let value = /name|title/.test(key) ? "测试名称" : "test";
  if (typeof schema.minLength === "number") value = value.padEnd(Math.min(schema.minLength, 256), "x");
  if (typeof schema.maxLength === "number") value = value.slice(0, Math.max(0, schema.maxLength));
  return value;
}
