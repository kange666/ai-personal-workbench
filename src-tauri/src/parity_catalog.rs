use crate::testing::{client_root, TestMenu};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path};
use std::process::Command;
use std::time::Duration;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct CatalogContract {
    pub platform: String,
    pub method: String,
    pub url: String,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct CatalogFeature {
    pub id: String,
    pub domain: String,
    pub name: String,
    pub pc_page: String,
    pub app_page: String,
    pub automatic_status: String,
    pub evidence: Vec<String>,
    pub pc_menu_ids: Vec<String>,
    pub app_menu_ids: Vec<String>,
    pub contracts: Vec<CatalogContract>,
}

#[derive(Debug)]
pub struct FullCatalog {
    pub features: Vec<CatalogFeature>,
    pub pc_count: usize,
    pub app_count: usize,
    pub matched_count: usize,
    pub pc_only_count: usize,
    pub app_only_count: usize,
    pub source_message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackendMenu {
    #[serde(default)]
    menu_id: i64,
    #[serde(default, deserialize_with = "null_string")]
    menu_name: String,
    #[serde(default)]
    parent_id: i64,
    #[serde(default, deserialize_with = "null_string")]
    path: String,
    #[serde(default, deserialize_with = "null_string")]
    component: String,
    #[serde(default, deserialize_with = "null_string")]
    menu_type: String,
    #[serde(default, deserialize_with = "null_string")]
    code: String,
    #[serde(default, deserialize_with = "null_string")]
    oper_url: String,
    #[serde(default, deserialize_with = "null_string")]
    request_mode: String,
}

fn null_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Deserialize)]
struct BackendMenuResponse {
    #[serde(default)]
    rows: Vec<BackendMenu>,
}

#[derive(Debug, Clone)]
struct FeatureNode {
    key: String,
    name: String,
    domain: String,
    parent_name: String,
    menu_type: String,
    route: String,
    source_path: String,
    code: String,
    method: String,
    api_url: String,
    menu_ids: Vec<String>,
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|ch| ch.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(ch))
        .collect()
}

fn normalized_route(value: &str) -> String {
    normalize(
        value
            .trim_start_matches('/')
            .trim_end_matches(".vue")
            .trim_end_matches("/index"),
    )
}

fn route_leaf(value: &str) -> String {
    value
        .trim_start_matches('/')
        .trim_end_matches(".vue")
        .split('/')
        .filter(|part| !matches!(*part, "index" | "pages" | "src" | "views"))
        .next_back()
        .map(normalize)
        .unwrap_or_default()
}

fn stable_hash(value: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn endpoint_from_client_config() -> Option<String> {
    if let Ok(value) = std::env::var("AI_WORKBENCH_MENU_API") {
        if value.starts_with("http://") || value.starts_with("https://") {
            return Some(value);
        }
    }
    let source = std::fs::read_to_string(client_root().join("vite.config.js")).ok()?;
    for line in source.lines() {
        if !line.contains("target:")
            || !line.contains("http")
            || line.trim_start().starts_with("//")
        {
            continue;
        }
        let start = line.find("http")?;
        let rest = &line[start..];
        let end = rest
            .find(|ch: char| matches!(ch, '`' | '\'' | '"' | ',') || ch.is_whitespace())
            .unwrap_or(rest.len());
        return Some(format!(
            "{}/api/menu/list",
            rest[..end].trim_end_matches('/')
        ));
    }
    None
}

fn fetch_backend_menus(port_flag: u8) -> Result<Vec<BackendMenu>, String> {
    let token =
        read_user_token().ok_or_else(|| "未读取到 Windows 用户环境变量 HLZT_TOKEN".to_string())?;
    if token.trim().is_empty() {
        return Err("Windows 用户环境变量 HLZT_TOKEN 为空".into());
    }
    let endpoint =
        endpoint_from_client_config().ok_or("无法从 client/vite.config.js 确定菜单接口")?;
    let response = Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|error| error.to_string())?
        .get(endpoint)
        .query(&[("portFlag", port_flag)])
        .header("hlzt-token", token)
        .send()
        .map_err(|error| format!("菜单接口请求失败：{error}"))?;
    if !response.status().is_success() {
        return Err(format!("菜单接口返回 HTTP {}", response.status()));
    }
    let bytes = response.bytes().map_err(|error| error.to_string())?;
    let payload: BackendMenuResponse =
        serde_json::from_slice(&bytes).map_err(|error| format!("菜单接口解析失败：{error}"))?;
    Ok(payload.rows)
}

fn read_user_token() -> Option<String> {
    if let Ok(value) = std::env::var("HLZT_TOKEN") {
        if !value.trim().is_empty() {
            return Some(value);
        }
    }
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("reg")
            .args(["query", r"HKCU\Environment", "/v", "HLZT_TOKEN"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        return stdout
            .lines()
            .find(|line| line.contains("HLZT_TOKEN"))
            .and_then(|line| line.split_whitespace().last())
            .map(str::to_string)
            .filter(|value| !value.trim().is_empty());
    }
    #[cfg(not(target_os = "windows"))]
    None
}

fn source_page(platform: &str, menu: &BackendMenu) -> String {
    if platform == "PC" {
        if menu.component.is_empty()
            || matches!(
                menu.component.as_str(),
                "Layout" | "ParentView" | "InnerLink"
            )
        {
            String::new()
        } else {
            format!("src/views/{}.vue", menu.component.trim_start_matches('/'))
        }
    } else {
        let path = if menu.path.starts_with("/pages/") {
            menu.path.trim_start_matches('/').to_string()
        } else if menu.component.starts_with("pages/") {
            menu.component.trim_start_matches('/').to_string()
        } else {
            String::new()
        };
        if path.is_empty() {
            path
        } else {
            format!("{}.vue", path.trim_end_matches(".vue"))
        }
    }
}

fn backend_nodes(platform: &str, menus: Vec<BackendMenu>) -> Vec<FeatureNode> {
    let by_id = menus
        .iter()
        .map(|item| (item.menu_id, item))
        .collect::<HashMap<_, _>>();
    menus
        .iter()
        .filter(|item| !item.menu_name.trim().is_empty())
        .map(|item| {
            let parent = by_id.get(&item.parent_id).copied();
            let mut root = item;
            let mut seen = HashSet::new();
            while root.parent_id != 0 && seen.insert(root.menu_id) {
                let Some(next) = by_id.get(&root.parent_id).copied() else {
                    break;
                };
                root = next;
            }
            let route = if !item.component.is_empty() {
                item.component.clone()
            } else if !item.path.is_empty() {
                item.path.clone()
            } else if !item.oper_url.is_empty() {
                item.oper_url.clone()
            } else {
                item.code.clone()
            };
            FeatureNode {
                key: format!("backend:{}:{}", platform.to_lowercase(), item.menu_id),
                name: item.menu_name.trim().to_string(),
                domain: root.menu_name.trim().to_string(),
                parent_name: parent
                    .map(|value| value.menu_name.trim().to_string())
                    .unwrap_or_default(),
                menu_type: if item.menu_type.is_empty() {
                    "C".into()
                } else {
                    item.menu_type.clone()
                },
                route,
                source_path: source_page(platform, item),
                code: item.code.clone(),
                method: item.request_mode.to_uppercase(),
                api_url: item.oper_url.clone(),
                menu_ids: Vec::new(),
            }
        })
        .collect()
}

fn is_internal_component(path: &Path) -> bool {
    path.components().any(|part| {
        let Component::Normal(value) = part else {
            return false;
        };
        matches!(
            value.to_string_lossy().to_ascii_lowercase().as_str(),
            "components" | "component" | "modules" | "plugin"
        )
    })
}

fn client_source_nodes() -> Vec<FeatureNode> {
    let root = client_root().join("src/views");
    WalkDir::new(&root)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("vue"))
        .filter(|entry| !is_internal_component(entry.path()))
        .filter_map(|entry| {
            let relative = entry.path().strip_prefix(client_root()).ok()?;
            let source = relative.to_string_lossy().replace('\\', "/");
            if source.contains(" copy") || source.contains("Test.vue") || source.contains("Old.vue")
            {
                return None;
            }
            let route = source
                .trim_start_matches("src/views/")
                .trim_end_matches(".vue")
                .to_string();
            let parts = route.split('/').collect::<Vec<_>>();
            let leaf = parts.last().copied().unwrap_or("页面");
            let name = if leaf.eq_ignore_ascii_case("index") {
                parts.iter().rev().nth(1).copied().unwrap_or("页面")
            } else {
                leaf
            };
            Some(FeatureNode {
                key: format!("source:pc:{route}"),
                name: name.to_string(),
                domain: parts.first().copied().unwrap_or("其他").to_string(),
                parent_name: parts
                    .iter()
                    .rev()
                    .nth(1)
                    .copied()
                    .unwrap_or_default()
                    .to_string(),
                menu_type: "C".into(),
                route,
                source_path: source,
                code: String::new(),
                method: String::new(),
                api_url: String::new(),
                menu_ids: Vec::new(),
            })
        })
        .collect()
}

fn attach_test_menus(nodes: &mut [FeatureNode], menus: &[TestMenu]) {
    for menu in menus {
        let menu_source = normalized_route(&menu.source_path);
        let menu_route = normalized_route(&menu.route);
        let menu_name = normalize(&menu.name);
        if let Some(node) = nodes.iter_mut().find(|node| {
            (!menu_source.is_empty() && normalized_route(&node.source_path) == menu_source)
                || (!menu_route.is_empty() && normalized_route(&node.route) == menu_route)
                || (normalize(&node.name) == menu_name && !menu_name.is_empty())
        }) {
            if !node.menu_ids.contains(&menu.id) {
                node.menu_ids.push(menu.id.clone());
            }
        }
    }
}

fn merge_source_pages(nodes: &mut Vec<FeatureNode>, source_nodes: Vec<FeatureNode>) {
    let mut known = nodes
        .iter()
        .filter_map(|item| {
            let value = normalized_route(&item.source_path);
            (!value.is_empty()).then_some(value)
        })
        .collect::<HashSet<_>>();
    for item in source_nodes {
        let source = normalized_route(&item.source_path);
        if !source.is_empty() && known.insert(source) {
            nodes.push(item);
        }
    }
}

fn app_page_nodes(menus: &[TestMenu]) -> Vec<FeatureNode> {
    menus
        .iter()
        .map(|menu| {
            let route = menu.route.trim_start_matches('/').to_string();
            let mut parts = route.split('/').filter(|part| !part.is_empty());
            let first = parts.nth(1).unwrap_or("APP");
            FeatureNode {
                key: format!("source:app:{route}"),
                name: menu.name.clone(),
                domain: first.trim_end_matches("Package").to_string(),
                parent_name: route
                    .split('/')
                    .rev()
                    .nth(1)
                    .unwrap_or_default()
                    .to_string(),
                menu_type: "C".into(),
                route,
                source_path: menu.source_path.clone(),
                code: String::new(),
                method: String::new(),
                api_url: String::new(),
                menu_ids: vec![menu.id.clone()],
            }
        })
        .collect()
}

fn generic_name(value: &str) -> bool {
    matches!(
        value,
        "新增"
            | "修改"
            | "删除"
            | "查询"
            | "查看"
            | "列表"
            | "详情"
            | "提交"
            | "审核"
            | "审批"
            | "导入"
            | "导出"
    )
}

fn known_pair_name(pc: &FeatureNode, app: &FeatureNode) -> Option<&'static str> {
    let pc_route = pc.route.to_ascii_lowercase();
    let app_route = app.route.to_ascii_lowercase();
    if pc_route.contains("safe/safetymanagement/caseshare/index")
        && app_route.contains("safepackage/pages/caseshare/index")
    {
        return Some("案例分享");
    }
    if pc_route.contains("workflow/task/taskwaiting")
        && app_route.contains("mainpackage/tabbar/mytask/index")
    {
        return Some("我的任务");
    }
    if pc_route.contains("system/message/index")
        && app_route.contains("mypackage/pages/message/index")
    {
        return Some("消息通知");
    }
    if pc_route.contains("system/user/index")
        && app_route.contains("resourcespackage/pages/usermanage/index")
    {
        return Some("用户管理");
    }
    None
}

fn match_score(
    pc: &FeatureNode,
    app: &FeatureNode,
    pc_name_counts: &HashMap<String, usize>,
    app_name_counts: &HashMap<String, usize>,
) -> i32 {
    if known_pair_name(pc, app).is_some() {
        return 500;
    }
    if pc.menu_type == "F" || app.menu_type == "F" {
        if pc.menu_type != app.menu_type {
            return 0;
        }
    }
    let pc_name = normalize(&pc.name);
    let app_name = normalize(&app.name);
    let same_name = !pc_name.is_empty() && pc_name == app_name;
    let same_parent = !normalize(&pc.parent_name).is_empty()
        && normalize(&pc.parent_name) == normalize(&app.parent_name);
    let mut score = 0;
    if same_name {
        score += 70;
        if pc_name_counts.get(&pc_name) == Some(&1) && app_name_counts.get(&app_name) == Some(&1) {
            score += 25;
        }
    }
    if same_parent {
        score += 35;
    }
    if !pc.code.is_empty() && pc.code == app.code {
        score += 100;
    }
    if !pc.api_url.is_empty() && pc.api_url == app.api_url {
        score += 110;
    }
    let pc_route = normalized_route(&pc.route);
    let app_route = normalized_route(&app.route);
    if !pc_route.is_empty() && pc_route == app_route {
        score += 100;
    }
    let pc_leaf = route_leaf(&pc.route);
    let app_leaf = route_leaf(&app.route);
    if !pc_leaf.is_empty() && pc_leaf == app_leaf && pc_leaf != "index" {
        score += 30;
    }
    if same_name && generic_name(&pc.name) && !same_parent && score < 100 {
        return 0;
    }
    score
}

fn contract(node: &FeatureNode, platform: &str) -> Option<CatalogContract> {
    if node.api_url.trim().is_empty() {
        return None;
    }
    Some(CatalogContract {
        platform: platform.into(),
        method: if node.method.is_empty() {
            "UNKNOWN".into()
        } else {
            node.method.clone()
        },
        url: node.api_url.clone(),
        source: node.route.clone(),
    })
}

fn build_feature(pc: Option<&FeatureNode>, app: Option<&FeatureNode>) -> CatalogFeature {
    let identity = format!(
        "{}|{}",
        pc.map(|item| item.key.as_str()).unwrap_or(""),
        app.map(|item| item.key.as_str()).unwrap_or("")
    );
    let name = match (pc, app) {
        (Some(pc), Some(app)) if known_pair_name(pc, app).is_some() => {
            known_pair_name(pc, app).unwrap_or("功能").to_string()
        }
        (Some(pc), Some(app)) if normalize(&pc.name) != normalize(&app.name) => {
            format!("{} ↔ {}", pc.name, app.name)
        }
        (Some(pc), _) => pc.name.clone(),
        (_, Some(app)) => app.name.clone(),
        _ => "未命名功能".into(),
    };
    let mut contracts = Vec::new();
    if let Some(value) = pc.and_then(|item| contract(item, "PC")) {
        contracts.push(value);
    }
    if let Some(value) = app.and_then(|item| contract(item, "APP")) {
        contracts.push(value);
    }
    CatalogFeature {
        id: format!("parity-auto-{}", stable_hash(&identity)),
        domain: pc
            .map(|item| item.domain.clone())
            .filter(|value| !value.is_empty())
            .or_else(|| app.map(|item| item.domain.clone()))
            .unwrap_or_else(|| "其他".into()),
        name,
        pc_page: pc.map(|item| item.source_path.clone()).unwrap_or_default(),
        app_page: app.map(|item| item.source_path.clone()).unwrap_or_default(),
        automatic_status: match (pc, app) {
            (Some(_), Some(_)) => "static-aligned",
            (Some(_), None) => "pc-only",
            (None, Some(_)) => "app-only",
            _ => "pending",
        }
        .into(),
        evidence: [
            pc.map(|item| format!("PC · {} · {}", item.name, item.route)),
            app.map(|item| format!("APP · {} · {}", item.name, item.route)),
        ]
        .into_iter()
        .flatten()
        .collect(),
        pc_menu_ids: pc.map(|item| item.menu_ids.clone()).unwrap_or_default(),
        app_menu_ids: app.map(|item| item.menu_ids.clone()).unwrap_or_default(),
        contracts,
    }
}

pub fn build_full_catalog(client_tests: &[TestMenu], app_tests: &[TestMenu]) -> FullCatalog {
    let pc_backend = fetch_backend_menus(1);
    let app_backend = fetch_backend_menus(2);
    let mut pc_nodes = pc_backend
        .as_ref()
        .map(|items| backend_nodes("PC", items.clone()))
        .unwrap_or_default();
    let mut app_nodes = app_backend
        .as_ref()
        .map(|items| backend_nodes("APP", items.clone()))
        .unwrap_or_default();
    merge_source_pages(&mut pc_nodes, client_source_nodes());
    merge_source_pages(&mut app_nodes, app_page_nodes(app_tests));
    attach_test_menus(&mut pc_nodes, client_tests);
    attach_test_menus(&mut app_nodes, app_tests);

    let pc_name_counts = pc_nodes.iter().fold(HashMap::new(), |mut map, item| {
        *map.entry(normalize(&item.name)).or_insert(0) += 1;
        map
    });
    let app_name_counts = app_nodes.iter().fold(HashMap::new(), |mut map, item| {
        *map.entry(normalize(&item.name)).or_insert(0) += 1;
        map
    });
    let mut used_app = HashSet::new();
    let mut features = Vec::new();
    let mut matched = 0;
    for pc in &pc_nodes {
        let best = app_nodes
            .iter()
            .enumerate()
            .filter(|(index, _)| !used_app.contains(index))
            .map(|(index, app)| {
                (
                    index,
                    match_score(pc, app, &pc_name_counts, &app_name_counts),
                )
            })
            .filter(|(_, score)| *score >= 60)
            .max_by_key(|(_, score)| *score);
        if let Some((index, _)) = best {
            used_app.insert(index);
            features.push(build_feature(Some(pc), Some(&app_nodes[index])));
            matched += 1;
        } else {
            features.push(build_feature(Some(pc), None));
        }
    }
    for (index, app) in app_nodes.iter().enumerate() {
        if !used_app.contains(&index) {
            features.push(build_feature(None, Some(app)));
        }
    }
    features.sort_by(|a, b| (&a.domain, &a.name).cmp(&(&b.domain, &b.name)));
    let pc_only_count = pc_nodes.len().saturating_sub(matched);
    let app_only_count = app_nodes.len().saturating_sub(matched);
    let source_message = match (&pc_backend, &app_backend) {
        (Ok(pc), Ok(app)) => format!(
            "已读取真实菜单：PC {} 条、APP {} 条，并合并两端源码页面",
            pc.len(),
            app.len()
        ),
        _ => {
            let reason = pc_backend
                .as_ref()
                .err()
                .or_else(|| app_backend.as_ref().err())
                .map(String::as_str)
                .unwrap_or("未知原因");
            format!(
                "菜单接口不可用（{}），已回退为本地源码：PC {} 条、APP {} 条",
                reason,
                pc_nodes.len(),
                app_nodes.len()
            )
        }
    };
    FullCatalog {
        features,
        pc_count: pc_nodes.len(),
        app_count: app_nodes.len(),
        matched_count: matched,
        pc_only_count,
        app_only_count,
        source_message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, parent: &str, kind: &str, route: &str) -> FeatureNode {
        FeatureNode {
            key: format!("{name}-{route}"),
            name: name.into(),
            domain: "测试".into(),
            parent_name: parent.into(),
            menu_type: kind.into(),
            route: route.into(),
            source_path: String::new(),
            code: String::new(),
            method: String::new(),
            api_url: String::new(),
            menu_ids: Vec::new(),
        }
    }

    #[test]
    fn duplicate_generic_actions_require_same_parent() {
        let pc = node("新增", "用户管理", "F", "user:add");
        let wrong = node("新增", "角色管理", "F", "role:add");
        let right = node("新增", "用户管理", "F", "user:add");
        let mut pc_counts = HashMap::new();
        let mut app_counts = HashMap::new();
        pc_counts.insert(normalize("新增"), 2);
        app_counts.insert(normalize("新增"), 2);
        assert_eq!(match_score(&pc, &wrong, &pc_counts, &app_counts), 0);
        assert!(match_score(&pc, &right, &pc_counts, &app_counts) >= 60);
    }

    #[test]
    fn unmatched_features_are_kept_on_their_platform() {
        let pc = node("用户管理", "系统管理", "C", "system/user/index");
        let app = node("账号安全", "我的", "C", "pages/my/account");
        assert_eq!(build_feature(Some(&pc), None).automatic_status, "pc-only");
        assert_eq!(build_feature(None, Some(&app)).automatic_status, "app-only");
    }
}
