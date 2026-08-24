use crate::database::DatabaseState;
use crate::parity_catalog::build_full_catalog;
use crate::testing::{app_menus, client_menus};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
    pub pc_feature_count: usize,
    pub app_feature_count: usize,
    pub matched_count: usize,
    pub pc_only_count: usize,
    pub app_only_count: usize,
    pub contract_count: usize,
    pub regression_count: usize,
    pub aligned_count: usize,
    pub pending_count: usize,
    pub source_message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveParityReview {
    pub id: String,
    pub parity_status: String,
    pub intentional_difference: bool,
    pub manually_confirmed: bool,
}

fn existing_run(
    connection: &Connection,
    menu_ids: &[String],
    mode: &str,
) -> Result<Option<(String, String)>, String> {
    if menu_ids.is_empty() {
        return Ok(None);
    }
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
    connection: &Connection,
    feature_id: &str,
    platform: &str,
    kind: &str,
    status: &str,
    summary: &str,
    source: &str,
    verified_at: Option<&str>,
) -> Result<(), String> {
    let id = format!("{feature_id}-{platform}-{kind}").to_lowercase();
    connection.execute(
        "INSERT INTO regression_cases(id,feature_id,platform,verification_type,case_name,status,result_summary,source_path,verified_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(id) DO UPDATE SET status=excluded.status,result_summary=excluded.result_summary,source_path=excluded.source_path,verified_at=excluded.verified_at",
        params![id, feature_id, platform, kind, format!("{platform} {kind}"), status, summary, source, verified_at],
    ).map_err(|error| error.to_string())?;
    Ok(())
}

pub fn sync_feature_parity_for_state(state: &DatabaseState) -> Result<ParitySyncSummary, String> {
    let client_catalog = client_menus(state)?;
    let app_catalog = app_menus(state)?;
    let catalog = build_full_catalog(&client_catalog, &app_catalog);
    let now = Utc::now().to_rfc3339();
    let mut contract_count = 0;
    let mut connection = state.connect()?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let active_ids = catalog
        .features
        .iter()
        .map(|feature| feature.id.clone())
        .collect::<HashSet<_>>();
    for feature in &catalog.features {
        let previous = transaction.query_row(
            "SELECT parity_status,intentional_difference,manually_confirmed FROM feature_parities WHERE id=?1",
            [&feature.id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)),
        ).optional().map_err(|error| error.to_string())?;
        let (status, intentional, confirmed) = previous
            .filter(|(_, _, confirmed)| *confirmed != 0)
            .unwrap_or_else(|| (feature.automatic_status.clone(), 0, 0));
        let evidence = serde_json::to_string(&feature.evidence).unwrap_or_else(|_| "[]".into());
        transaction.execute(
            "INSERT INTO feature_parities(id,domain,feature_name,pc_page,app_page,parity_status,evidence_json,intentional_difference,manually_confirmed,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) ON CONFLICT(id) DO UPDATE SET domain=excluded.domain,feature_name=excluded.feature_name,pc_page=excluded.pc_page,app_page=excluded.app_page,evidence_json=excluded.evidence_json,parity_status=?6,intentional_difference=?8,manually_confirmed=?9,updated_at=excluded.updated_at",
            params![feature.id, feature.domain, feature.name, feature.pc_page, feature.app_page, status, evidence, intentional, confirmed, now],
        ).map_err(|error| error.to_string())?;
        transaction
            .execute(
                "DELETE FROM api_contracts WHERE feature_id=?1",
                [&feature.id],
            )
            .map_err(|error| error.to_string())?;
        for (index, contract) in feature.contracts.iter().enumerate() {
            let id = format!(
                "{}-{}-{index}",
                feature.id,
                contract.platform.to_lowercase()
            );
            transaction.execute(
                "INSERT INTO api_contracts(id,feature_id,platform,method,url,parameters_json,response_fields_json,source_file,verification_level,updated_at) VALUES(?1,?2,?3,?4,?5,'{}','[]',?6,'static',?7)",
                params![id, feature.id, contract.platform, contract.method, contract.url, contract.source, now],
            ).map_err(|error| error.to_string())?;
            contract_count += 1;
        }
        let has_pc = feature.automatic_status != "app-only";
        let has_app = feature.automatic_status != "pc-only";
        upsert_regression(
            &transaction,
            &feature.id,
            "PC",
            "static",
            if has_pc { "passed" } else { "unverified" },
            if has_pc {
                "已进入 PC 真实菜单或本地源码全量清单"
            } else {
                "PC 侧未找到对应功能"
            },
            &feature.pc_page,
            has_pc.then_some(now.as_str()),
        )?;
        upsert_regression(
            &transaction,
            &feature.id,
            "APP",
            "static",
            if has_app { "passed" } else { "unverified" },
            if has_app {
                "已进入 APP 真实菜单或 pages.json 全量清单"
            } else {
                "APP 侧未找到对应功能"
            },
            &feature.app_page,
            has_app.then_some(now.as_str()),
        )?;
        let real = existing_run(&transaction, &feature.pc_menu_ids, "real")?;
        let browser = existing_run(&transaction, &feature.pc_menu_ids, "browser-style")?;
        upsert_regression(
            &transaction,
            &feature.id,
            "PC",
            "api",
            real.as_ref().map(|v| v.0.as_str()).unwrap_or("unverified"),
            if real.is_some() {
                "来自工作台保存的真实接口测试"
            } else {
                "尚未执行真实接口测试"
            },
            feature
                .pc_menu_ids
                .first()
                .map(String::as_str)
                .unwrap_or(""),
            real.as_ref().map(|v| v.1.as_str()),
        )?;
        upsert_regression(
            &transaction,
            &feature.id,
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
            feature
                .pc_menu_ids
                .first()
                .map(String::as_str)
                .unwrap_or(""),
            browser.as_ref().map(|v| v.1.as_str()),
        )?;
        let app_source = existing_run(&transaction, &feature.app_menu_ids, "source-style")?;
        let app_browser = existing_run(&transaction, &feature.app_menu_ids, "browser-style")?;
        if let Some((status, verified_at)) = app_source.as_ref() {
            upsert_regression(
                &transaction,
                &feature.id,
                "APP",
                "static",
                status,
                "来自工作台保存的 APP 源码与样式检查",
                feature
                    .app_menu_ids
                    .first()
                    .map(String::as_str)
                    .unwrap_or(""),
                Some(verified_at),
            )?;
        }
        upsert_regression(
            &transaction,
            &feature.id,
            "APP",
            "api",
            "unverified",
            if has_app {
                "尚未执行 APP 真实接口测试"
            } else {
                "APP 侧无对应功能"
            },
            feature
                .app_menu_ids
                .first()
                .map(String::as_str)
                .unwrap_or(""),
            None,
        )?;
        upsert_regression(
            &transaction,
            &feature.id,
            "APP",
            "browser",
            app_browser
                .as_ref()
                .map(|value| value.0.as_str())
                .unwrap_or("unverified"),
            if app_browser.is_some() {
                "来自工作台保存的 APP 浏览器测试"
            } else if has_app {
                "尚未执行 APP 浏览器测试"
            } else {
                "APP 侧无对应功能"
            },
            feature
                .app_menu_ids
                .first()
                .map(String::as_str)
                .unwrap_or(""),
            app_browser.as_ref().map(|value| value.1.as_str()),
        )?;
    }
    for legacy_id in [
        "parity-case-share",
        "parity-workflow-task",
        "parity-message",
        "parity-user",
    ] {
        let legacy = transaction.query_row(
            "SELECT feature_name,parity_status,intentional_difference,manually_confirmed FROM feature_parities WHERE id=?1",
            [legacy_id],
            |row| Ok((row.get::<_, String>(0)?,row.get::<_, String>(1)?,row.get::<_, i64>(2)?,row.get::<_, i64>(3)?)),
        ).optional().map_err(|error| error.to_string())?;
        if let Some((name, status, intentional, confirmed)) = legacy {
            if confirmed != 0 {
                transaction.execute(
                    "UPDATE feature_parities SET parity_status=?2,intentional_difference=?3,manually_confirmed=1,updated_at=?4 WHERE id=(SELECT id FROM feature_parities WHERE id LIKE 'parity-auto-%' AND feature_name=?1 ORDER BY CASE parity_status WHEN 'static-aligned' THEN 0 ELSE 1 END LIMIT 1)",
                    params![name,status,intentional,now],
                ).map_err(|error| error.to_string())?;
            }
            transaction
                .execute("DELETE FROM api_contracts WHERE feature_id=?1", [legacy_id])
                .map_err(|error| error.to_string())?;
            transaction
                .execute(
                    "DELETE FROM regression_cases WHERE feature_id=?1",
                    [legacy_id],
                )
                .map_err(|error| error.to_string())?;
            transaction
                .execute("DELETE FROM feature_parities WHERE id=?1", [legacy_id])
                .map_err(|error| error.to_string())?;
        }
    }
    let stale_ids = {
        let mut statement = transaction
            .prepare("SELECT id FROM feature_parities WHERE id LIKE 'parity-auto-%' AND manually_confirmed=0")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        rows
    };
    for id in stale_ids.into_iter().filter(|id| !active_ids.contains(id)) {
        transaction
            .execute("DELETE FROM api_contracts WHERE feature_id=?1", [&id])
            .map_err(|error| error.to_string())?;
        transaction
            .execute("DELETE FROM regression_cases WHERE feature_id=?1", [&id])
            .map_err(|error| error.to_string())?;
        transaction
            .execute("DELETE FROM feature_parities WHERE id=?1", [&id])
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    let records = list_feature_parity_for_state(state)?;
    Ok(ParitySyncSummary {
        feature_count: records.len(),
        pc_feature_count: catalog.pc_count,
        app_feature_count: catalog.app_count,
        matched_count: catalog.matched_count,
        pc_only_count: catalog.pc_only_count,
        app_only_count: catalog.app_only_count,
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
        source_message: catalog.source_message,
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
    if ![
        "pending",
        "static-aligned",
        "confirmed",
        "different",
        "pc-only",
        "app-only",
    ]
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
    fn real_projects_create_full_feature_matrix_without_faking_live_results() {
        let path =
            std::env::temp_dir().join(format!("workbench-parity-{}.sqlite3", Uuid::new_v4()));
        let state = DatabaseState::new(path.clone()).unwrap();
        let summary = sync_feature_parity_for_state(&state).unwrap();
        println!(
            "full parity: PC={}, APP={}, matched={}, PC-only={}, APP-only={}, total={}, source={}",
            summary.pc_feature_count,
            summary.app_feature_count,
            summary.matched_count,
            summary.pc_only_count,
            summary.app_only_count,
            summary.feature_count,
            summary.source_message
        );
        assert!(summary.pc_feature_count > 100);
        assert!(summary.app_feature_count > 100);
        assert!(summary.feature_count >= summary.pc_feature_count);
        assert_eq!(
            summary.feature_count,
            summary.matched_count + summary.pc_only_count + summary.app_only_count
        );
        let records = list_feature_parity_for_state(&state).unwrap();
        for expected in ["案例分享", "我的任务", "消息通知", "用户管理"] {
            assert!(
                records.iter().any(|item| {
                    item.feature_name == expected
                        && !matches!(item.parity_status.as_str(), "pc-only" | "app-only")
                }),
                "{expected} 应当在 PC 与 APP 之间匹配"
            );
        }
        assert!(records.iter().all(|item| item
            .regression
            .iter()
            .filter(|check| check.verification_type != "static")
            .all(|check| check.status == "unverified")));
        assert!(records.iter().all(|item| {
            item.regression
                .iter()
                .filter(|check| check.verification_type == "static")
                .count()
                == 2
        }));
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
        let id: String = state
            .connect()
            .unwrap()
            .query_row("SELECT id FROM feature_parities LIMIT 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        state.connect().unwrap().execute("UPDATE feature_parities SET parity_status='different',intentional_difference=1,manually_confirmed=1 WHERE id=?1", [&id]).unwrap();
        sync_feature_parity_for_state(&state).unwrap();
        let message = list_feature_parity_for_state(&state)
            .unwrap()
            .into_iter()
            .find(|item| item.id == id)
            .unwrap();
        assert_eq!(message.parity_status, "different");
        assert!(message.intentional_difference);
        assert!(message.manually_confirmed);
        drop(state);
        let _ = std::fs::remove_file(path);
    }
}
