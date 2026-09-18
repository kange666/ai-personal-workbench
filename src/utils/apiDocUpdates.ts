export interface ApiUpdateState {
  known: Record<string,string>;
  unread: Record<string,"added"|"updated">;
}
const key = (source:string) => `workbench-api-doc-updates-v1:${source}`;

export function readApiUpdates(source:string):ApiUpdateState|null {
  try {
    const value=JSON.parse(localStorage.getItem(key(source)) || "null");
    if(!value || typeof value.known!=="object" || !value.known || typeof value.unread!=="object" || !value.unread) return null;
    return {
      known:Object.fromEntries(Object.entries(value.known).filter((entry):entry is [string,string]=>typeof entry[1]==="string")),
      unread:Object.fromEntries(Object.entries(value.unread).filter((entry):entry is [string,"added"|"updated"]=>entry[1]==="added" || entry[1]==="updated")),
    };
  } catch { return null; }
}
export function saveApiUpdates(source:string,state:ApiUpdateState) {
  // 只保存本地已读标记与内容指纹，不保存接口正文或凭据。
  try { localStorage.setItem(key(source),JSON.stringify(state)); } catch { /* 存储不可用时当前页面仍可提示。 */ }
}
export function observeApiUpdates(previous:ApiUpdateState|null,items:{id:string;documentHash:string}[]):ApiUpdateState {
  const known:Record<string,string>={}, unread:ApiUpdateState["unread"]={};
  for(const item of items) {
    known[item.id]=item.documentHash;
    if(!previous) continue; // 首次启用时将已有缓存作为基线，不把历史文档全部标为新增。
    if(previous.unread[item.id]) unread[item.id]=previous.unread[item.id];
    if(!(item.id in previous.known)) unread[item.id]="added";
    else if(previous.known[item.id]!==item.documentHash) unread[item.id]=unread[item.id]==="added" ? "added":"updated";
  }
  return {known,unread};
}
