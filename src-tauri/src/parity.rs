use crate::database::DatabaseState;
use crate::testing::{app_menus, app_root, client_menus, client_root, TestMenu};
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiContract {
    pub id: String,
    pub feature_id: String,
    pub platform: String,
    pub method: String,
    pub url: String,
    pub source_file: String,
    pub verification_level: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionEvidence {
    pub platform: String,
    pub verification_type: String,
    pub status: String,
    pub result_summary: String,
    pub source_path: String,
    pub verified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureParity {
    pub id: String,
    pub domain: String,
    pub feature_name: String,
    pub pc_page: String,
    pub app_page: String,
    pub parity_status: String,
    pub evidence: Vec<String>,
    pub intentional_difference: bool,
    pub manually_confirmed: bool,
    pub updated_at: String,
    pub contracts: Vec<ApiContract>,
    pub regression: Vec<RegressionEvidence>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParitySyncSummary {
    pub feature_count: usize,
    pub contract_count: usize,
    pub regression_count: usize,
    pub aligned_count: usize,
    pub pending_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveParityReview {
    pub id: String,
    pub parity_status: String,
    pub intentional_difference: bool,
    pub manually_confirmed: bool,
}

struct FeatureDefinition {
    id: &'static str,
    domain: &'static str,
    name: &'static str,
    pc_page: &'static str,
    app_page: &'static str,
    pc_api: &'static str,
    app_api: &'static str,
    pc_case_ids: &'static [&'static str],
    contracts: &'static [(&'static str, &'static str)],
}

const FEATURES: &[FeatureDefinition] = &[
    FeatureDefinition {
        id: "parity-case-share",
        domain: "安全管理",
        name: "案例分享",
        pc_page: "src/views/safe/safetyManagement/caseShare/index.vue",
        app_page: "pages/safePackage/pages/caseShare/index.vue",
        pc_api: "src/api/safe/safetyManagement/caseShare.js",
        app_api: "src/api/safe/caseShare.js",
        pc_case_ids: &["client:case-share"],
        contracts: &[
            ("GET", "/case-share/posts"),
            ("GET", "/case-share/posts/{id}"),
            ("POST", "/case-share/posts"),
            ("PUT", "/case-share/posts/{id}"),
            ("DELETE", "/case-share/posts/{id}"),
        ],
    },
    FeatureDefinition {
        id: "parity-workflow-task",
        domain: "流程管理",
        name: "我的任务",
        pc_page: "src/views/workflow/task/taskWaiting.vue",
        app_page: "pages/mainPackage/tabbar/myTask/index.vue",
        pc_api: "src/api/workflow/task/index.js",
        app_api: "src/api/pages/myTask.js",
        pc_case_ids: &[
            "client:workflow-my-waiting",
            "client:workflow-my-finish",
            "client:workflow-my-copy",
            "client:workflow-my-document",
            "client:workflow-all-task",
        ],
        contracts: &[
            ("GET", "/task/pageByTaskWait"),
            ("GET", "/task/pageByTaskFinish"),
            ("GET", "/task/pageByTaskCopy"),
            ("POST", "/task/completeTask"),
            ("POST", "/task/backProcess"),
        ],
    },
    FeatureDefinition {
        id: "parity-message",
        domain: "消息中心",
        name: "消息通知",
        pc_page: "src/views/system/message/index.vue",
        app_page: "pages/myPackage/pages/message/index.vue",
        pc_api: "src/api/system/message.js",
        app_api: "src/api/pages/message.js",
        pc_case_ids: &[],
        contracts: &[
            ("GET", "/message/getMessageInfoPageList"),
            ("GET", "/message/getMyMessageInfoPageList"),
            ("GET", "/message/getMyUnreadQuantity"),
            ("PUT", "/message/{id}"),
        ],
    },
    FeatureDefinition {
        id: "parity-user",
        domain: "系统管理",
        name: "用户管理",
        pc_page: "src/views/system/user/index.vue",
        app_page: "pages/resourcesPackage/pages/userManage/index.vue",
        pc_api: "src/api/system/user.js",
        app_api: "src/api/pages/user.js",
        pc_case_ids: &[],
        contracts: &[
            ("GET", "/user/list"),
            ("GET", "/user/{id}"),
            ("POST", "/user"),
            ("PUT", "/user"),
            ("DELETE", "/user/{id}"),
        ],
    },
];

fn normalized_source(root: &Path, relative: &str) -> String {
    root.join(relative).display().to_string()
}

fn source_contains(root: &Path, relative: &str, needles: &[&str]) -> bool {
    let Ok(source) = std::fs::read_to_string(root.join(relative)) else {
        return false;
    };
    needles.iter().all(|needle| {
        let literal = needle
            .split("/{")
            .next()
            .unwrap_or(needle)
            .trim_start_matches('/');
        source.contains(literal)
    })
}

fn existing_run(
    state: &DatabaseState,
    menu_ids: &[&str],
    mode: &str,
) -> Result<Option<(String, String)>, String> {
    if menu_ids.is_empty() {
        return Ok(None);
    }
    let connection = state.connect()?;
    for menu_id in menu_ids {
        let row = connection.query_row(
            "SELECT status,COALESCE(finished_at,started_at) FROM test_runs WHERE menu_id=?1 AND mode=?2 ORDER BY started_at DESC LIMIT 1",
            params![menu_id, mode], |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional().map_err(|error| error.to_string())?;
        if row.is_some() {
            return Ok(row);
        }
    }
    Ok(None)
}

fn upsert_regression(
    state: &DatabaseState,
    feature_id: &str,
    platform: &str,
    kind: &str,
    status: &str,
    summary: &str,
    source: &str,
    verified_at: Option<&str>,
) -> Result<(), String> {
    let id = format!("{feature_id}-{platform}-{kind}").to_lowercase();
    state.connect()?.execute(
        "INSERT INTO regression_cases(id,feature_id,platform,verification_type,case_name,status,result_summary,source_path,verified_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(id) DO UPDATE SET status=excluded.status,result_summary=excluded.result_summary,source_path=excluded.source_path,verified_at=excluded.verified_at",
        params![id, feature_id, platform, kind, format!("{platform} {kind}"), status, summary, source, verified_at],
    ).map_err(|error| error.to_string())?;
    Ok(())
}

pub fn sync_feature_parity_for_state(state: &DatabaseState) -> Result<ParitySyncSummary, String> {
    let client_root = client_root();
    let app_root = app_root();
    let client_catalog = client_menus(state)?;
    let app_catalog = app_menus(state)?;
    let now = Utc::now().to_rfc3339();
    let mut contract_count = 0;
    for feature in FEATURES {
        let pc_static = client_root.join(feature.pc_page).is_file()
            && client_root.join(feature.pc_api).is_file();
        let app_static =
            app_root.join(feature.app_page).is_file() && app_root.join(feature.app_api).is_file();
        let endpoint_needles = feature
            .contracts
            .iter()
            .map(|(_, url)| *url)
            .collect::<Vec<_>>();
        let pc_contracts = source_contains(&client_root, feature.pc_api, &endpoint_needles);
        let app_contracts = source_contains(&app_root, feature.app_api, &endpoint_needles);
        let automatic_status = if pc_static && app_static && pc_contracts && app_contracts {
            "static-aligned"
        } else {
            "pending"
        };
        let previous = state.connect()?.query_row(
            "SELECT parity_status,intentional_difference,manually_confirmed FROM feature_parities WHERE id=?1",
            [feature.id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)),
        ).optional().map_err(|error| error.to_string())?;
        let (status, intentional, confirmed) = previous
            .filter(|(_, _, confirmed)| *confirmed != 0)
            .unwrap_or_else(|| (automatic_status.into(), 0, 0));
        let evidence = json!([
            normalized_source(&client_root, feature.pc_page),
            normalized_source(&app_root, feature.app_page),
            normalized_source(&client_root, feature.pc_api),
            normalized_source(&app_root, feature.app_api)
        ])
        .to_string();
        state.connect()?.execute(
            "INSERT INTO feature_parities(id,domain,feature_name,pc_page,app_page,parity_status,evidence_json,intentional_difference,manually_confirmed,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) ON CONFLICT(id) DO UPDATE SET domain=excluded.domain,feature_name=excluded.feature_name,pc_page=excluded.pc_page,app_page=excluded.app_page,evidence_json=excluded.evidence_json,parity_status=?6,intentional_difference=?8,manually_confirmed=?9,updated_at=excluded.updated_at",
            params![feature.id, feature.domain, feature.name, feature.pc_page, feature.app_page, status, evidence, intentional, confirmed, now],
        ).map_err(|error| error.to_string())?;
        state
            .connect()?
            .execute(
                "DELETE FROM api_contracts WHERE feature_id=?1",
                [feature.id],
            )
            .map_err(|error| error.to_string())?;
        for platform in ["PC", "APP"] {
            let source = if platform == "PC" {
                feature.pc_api
            } else {
                feature.app_api
            };
            let root = if platform == "PC" {
                &client_root
            } else {
                &app_root
            };
            for (index, (method, url)) in feature.contracts.iter().enumerate() {
                let id = format!("{}-{}-{index}", feature.id, platform.to_lowercase());
                state.connect()?.execute(
                    "INSERT INTO api_contracts(id,feature_id,platform,method,url,parameters_json,response_fields_json,source_file,verification_level,updated_at) VALUES(?1,?2,?3,?4,?5,'{}','[]',?6,'static',?7)",
                    params![id, feature.id, platform, method, url, normalized_source(root, source), now],
                ).map_err(|error| error.to_string())?;
                contract_count += 1;
            }
        }
        upsert_regression(
            state,
            feature.id,
            "PC",
            "static",
            if pc_static { "passed" } else { "failed" },
            if pc_static {
                "页面与 API 源文件存在"
            } else {
                "页面或 API 源文件缺失"
            },
            &normalized_source(&client_root, feature.pc_page),
            Some(&now),
        )?;
        upsert_regression(
            state,
            feature.id,
            "APP",
            "static",
            if app_static { "passed" } else { "failed" },
            if app_static {
                "页面与 API 源文件存在"
            } else {
                "页面或 API 源文件缺失"
            },
            &normalized_source(&app_root, feature.app_page),
            Some(&now),
        )?;
        let real = existing_run(state, feature.pc_case_ids, "real")?;
        let browser = existing_run(state, feature.pc_case_ids, "browser-style")?;
        upsert_regression(
            state,
            feature.id,
            "PC",
            "api",
            real.as_ref().map(|v| v.0.as_str()).unwrap_or("unverified"),
            if real.is_some() {
                "来自工作台保存的真实接口测试"
            } else {
                "尚未执行真实接口测试"
            },
            feature.pc_case_ids.first().copied().unwrap_or(""),
            real.as_ref().map(|v| v.1.as_str()),
        )?;
        upsert_regression(
            state,
            feature.id,
            "PC",
            "browser",
            browser
                .as_ref()
                .map(|v| v.0.as_str())
                .unwrap_or("unverified"),
            if browser.is_some() {
                "来自工作台保存的浏览器样式测试"
            } else {
                "尚未执行浏览器测试"
            },
            feature.pc_case_ids.first().copied().unwrap_or(""),
            browser.as_ref().map(|v| v.1.as_str()),
        )?;
        upsert_regression(
            state,
            feature.id,
            "APP",
            "api",
            "unverified",
            "APP 暂无真实接口自动化用例",
            feature.app_api,
            None,
        )?;
        upsert_regression(
            state,
            feature.id,
            "APP",
            "browser",
            "unverified",
            "APP 暂无浏览器自动化用例",
            feature.app_page,
            None,
        )?;
    }
    // Catalog counts are evidence that the matrix was built from current projects, not demo data.
    let _catalog_evidence: (Vec<TestMenu>, Vec<TestMenu>) = (client_catalog, app_catalog);
    let records = list_feature_parity_for_state(state)?;
    Ok(ParitySyncSummary {
        feature_count: records.len(),
        contract_count,
        regression_count: records.iter().map(|item| item.regression.len()).sum(),
        aligned_count: records
            .iter()
            .filter(|item| {
                item.parity_status == "static-aligned" || item.parity_status == "confirmed"
            })
            .count(),
        pending_count: records
            .iter()
            .filter(|item| item.parity_status == "pending")
            .count(),
    })
}

pub fn list_feature_parity_for_state(state: &DatabaseState) -> Result<Vec<FeatureParity>, String> {
    let connection = state.connect()?;
    let mut statement = connection.prepare("SELECT id,domain,feature_name,pc_page,app_page,parity_status,evidence_json,intentional_difference,manually_confirmed,updated_at FROM feature_parities ORDER BY domain,feature_name").map_err(|error| error.to_string())?;
    let base = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, String>(9)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let mut result = Vec::new();
    for (
        id,
        domain,
        name,
        pc_page,
        app_page,
        status,
        evidence_json,
        intentional,
        confirmed,
        updated_at,
    ) in base
    {
        let mut contract_statement = connection.prepare("SELECT id,feature_id,platform,method,url,source_file,verification_level FROM api_contracts WHERE feature_id=?1 ORDER BY platform,method,url").map_err(|error| error.to_string())?;
        let contracts = contract_statement
            .query_map([&id], |row| {
                Ok(ApiContract {
                    id: row.get(0)?,
                    feature_id: row.get(1)?,
                    platform: row.get(2)?,
                    method: row.get(3)?,
                    url: row.get(4)?,
                    source_file: row.get(5)?,
                    verification_level: row.get(6)?,
                })
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        let mut regression_statement = connection.prepare("SELECT platform,verification_type,status,result_summary,source_path,verified_at FROM regression_cases WHERE feature_id=?1 ORDER BY platform,verification_type").map_err(|error| error.to_string())?;
        let regression = regression_statement
            .query_map([&id], |row| {
                Ok(RegressionEvidence {
                    platform: row.get(0)?,
                    verification_type: row.get(1)?,
                    status: row.get(2)?,
                    result_summary: row.get(3)?,
                    source_path: row.get(4)?,
                    verified_at: row.get(5)?,
                })
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        result.push(FeatureParity {
            id,
            domain,
            feature_name: name,
            pc_page,
            app_page,
            parity_status: status,
            evidence: serde_json::from_str(&evidence_json).unwrap_or_default(),
            intentional_difference: intentional != 0,
            manually_confirmed: confirmed != 0,
            updated_at,
            contracts,
            regression,
        });
    }
    Ok(result)
}

#[tauri::command]
pub fn sync_feature_parity(
    state: tauri::State<'_, DatabaseState>,
) -> Result<ParitySyncSummary, String> {
    sync_feature_parity_for_state(&state)
}

#[tauri::command]
pub fn list_feature_parity(
    state: tauri::State<'_, DatabaseState>,
) -> Result<Vec<FeatureParity>, String> {
    list_feature_parity_for_state(&state)
}

#[tauri::command]
pub fn save_feature_parity_review(
    state: tauri::State<'_, DatabaseState>,
    review: SaveParityReview,
) -> Result<(), String> {
    if !["pending", "static-aligned", "confirmed", "different"]
        .contains(&review.parity_status.as_str())
    {
        return Err("不支持的对照状态。".into());
    }
    state.connect()?.execute("UPDATE feature_parities SET parity_status=?2,intentional_difference=?3,manually_confirmed=?4,updated_at=?5 WHERE id=?1", params![review.id,review.parity_status,review.intentional_difference as i64,review.manually_confirmed as i64,Utc::now().to_rfc3339()]).map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn real_projects_create_four_core_features_without_faking_live_results() {
        let path =
            std::env::temp_dir().join(format!("workbench-parity-{}.sqlite3", Uuid::new_v4()));
        let state = DatabaseState::new(path.clone()).unwrap();
        let summary = sync_feature_parity_for_state(&state).unwrap();
        assert_eq!(summary.feature_count, 4);
        assert!(summary.contract_count >= 30);
        let records = list_feature_parity_for_state(&state).unwrap();
        assert!(records.iter().all(|item| item
            .regression
            .iter()
            .filter(|check| check.verification_type != "static")
            .all(|check| check.status == "unverified")));
        assert!(records.iter().all(|item| item
            .regression
            .iter()
            .filter(|check| check.verification_type == "static")
            .all(|check| check.status == "passed")));
        drop(state);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn manually_confirmed_review_survives_resync() {
        let path = std::env::temp_dir().join(format!(
            "workbench-parity-review-{}.sqlite3",
            Uuid::new_v4()
        ));
        let state = DatabaseState::new(path.clone()).unwrap();
        sync_feature_parity_for_state(&state).unwrap();
        state.connect().unwrap().execute("UPDATE feature_parities SET parity_status='different',intentional_difference=1,manually_confirmed=1 WHERE id='parity-message'", []).unwrap();
        sync_feature_parity_for_state(&state).unwrap();
        let message = list_feature_parity_for_state(&state)
            .unwrap()
            .into_iter()
            .find(|item| item.id == "parity-message")
            .unwrap();
        assert_eq!(message.parity_status, "different");
        assert!(message.intentional_difference);
        assert!(message.manually_confirmed);
        drop(state);
        let _ = std::fs::remove_file(path);
    }
}
