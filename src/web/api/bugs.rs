//! Bug-report intake and AI auto-fix endpoints.
//!
//! - `submit_bug_report` is **public** (no auth extractor) and rate-limited per
//!   IP — it backs the `/report-bug` page.
//! - All other handlers require `AdminOnly` and drive triage + the AI fix
//!   pipeline.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::core::{AuditEntry, BugJob, BugReport};
use crate::web::auth::AdminOnly;
use crate::web::state::AppState;

const MAX_TITLE: usize = 200;
const MAX_TEXT: usize = 8000;
const VALID_SEVERITIES: &[&str] = &["low", "normal", "high", "critical"];
const VALID_STATUSES: &[&str] = &[
    "new",
    "triaged",
    "approved",
    "in_progress",
    "completed",
    "failed",
    "rejected",
];

// ---- Public submission ----

#[derive(Deserialize)]
pub struct SubmitBugBody {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub steps: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub reporter_email: Option<String>,
}

/// Extract the best-effort client IP from proxy headers, falling back to a
/// sentinel. The app runs behind Apache/DirectAdmin which sets X-Forwarded-For.
fn client_ip(headers: &HeaderMap) -> String {
    if let Some(xff) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = xff.split(',').next() {
            let ip = first.trim();
            if !ip.is_empty() {
                return ip.to_string();
            }
        }
    }
    if let Some(real) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        if !real.trim().is_empty() {
            return real.trim().to_string();
        }
    }
    "unknown".to_string()
}

/// POST /api/v1/bug-reports — public, rate-limited bug submission.
pub async fn submit_bug_report(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<SubmitBugBody>,
) -> impl IntoResponse {
    let title = body.title.trim();
    if title.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Title is required"})),
        )
            .into_response();
    }
    if title.len() > MAX_TITLE || body.description.len() > MAX_TEXT || body.steps.len() > MAX_TEXT {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Input too long"})),
        )
            .into_response();
    }

    let ip = client_ip(&headers);

    // Sliding-window in-memory rate limit per IP.
    let limit = state.config.aibug.rate_limit_per_hour.max(1) as usize;
    {
        let now = std::time::Instant::now();
        let window = std::time::Duration::from_secs(3600);
        let mut map = state.bug_report_rate.write().await;
        let entry = map.entry(ip.clone()).or_default();
        entry.retain(|t| now.duration_since(*t) < window);
        if entry.len() >= limit {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({"error": "Too many submissions. Please try again later."})),
            )
                .into_response();
        }
        entry.push(now);
    }

    let severity = if VALID_SEVERITIES.contains(&body.severity.as_str()) {
        body.severity.clone()
    } else {
        "normal".to_string()
    };
    let email = body
        .reporter_email
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.len() <= MAX_TITLE);

    let report = BugReport {
        id: 0,
        title: title.to_string(),
        description: body.description.trim().to_string(),
        steps: body.steps.trim().to_string(),
        severity,
        reporter_email: email,
        status: "new".to_string(),
        ip_address: Some(ip),
        admin_notes: String::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    match state.storage.create_bug_report(&report).await {
        Ok(id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({"id": id, "status": "received"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to save report: {e}")})),
        )
            .into_response(),
    }
}

// ---- Admin triage ----

#[derive(Deserialize)]
pub struct ListQuery {
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// GET /api/v1/bug-reports — list reports (admin).
pub async fn list_bug_reports(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(100).min(500);
    let offset = q.offset.unwrap_or(0);
    let status = q.status.as_deref().filter(|s| VALID_STATUSES.contains(s));
    let reports = state
        .storage
        .list_bug_reports(status, limit, offset)
        .await
        .unwrap_or_default();
    Json(serde_json::json!({ "reports": reports }))
}

/// GET /api/v1/bug-reports/:id — single report with its jobs (admin).
pub async fn get_bug_report(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.storage.get_bug_report(id).await {
        Ok(report) => {
            let jobs = state
                .storage
                .list_bug_jobs(Some(id), 50, 0)
                .await
                .unwrap_or_default();
            Json(serde_json::json!({ "report": report, "jobs": jobs })).into_response()
        }
        Err(crate::storage::StorageError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct UpdateBugBody {
    pub title: Option<String>,
    pub description: Option<String>,
    pub steps: Option<String>,
    pub severity: Option<String>,
    pub status: Option<String>,
    pub admin_notes: Option<String>,
}

/// PUT /api/v1/bug-reports/:id — edit a report (admin).
pub async fn update_bug_report(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateBugBody>,
) -> impl IntoResponse {
    let mut report = match state.storage.get_bug_report(id).await {
        Ok(r) => r,
        Err(crate::storage::StorageError::NotFound) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    if let Some(t) = body.title {
        let t = t.trim();
        if t.is_empty() || t.len() > MAX_TITLE {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid title"})),
            )
                .into_response();
        }
        report.title = t.to_string();
    }
    if let Some(d) = body.description {
        if d.len() > MAX_TEXT {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Description too long"})),
            )
                .into_response();
        }
        report.description = d;
    }
    if let Some(s) = body.steps {
        if s.len() > MAX_TEXT {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Steps too long"})),
            )
                .into_response();
        }
        report.steps = s;
    }
    if let Some(sev) = body.severity {
        if !VALID_SEVERITIES.contains(&sev.as_str()) {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid severity"})),
            )
                .into_response();
        }
        report.severity = sev;
    }
    if let Some(st) = body.status {
        if !VALID_STATUSES.contains(&st.as_str()) {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid status"})),
            )
                .into_response();
        }
        report.status = st;
    }
    if let Some(n) = body.admin_notes {
        if n.len() > MAX_TEXT {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Notes too long"})),
            )
                .into_response();
        }
        report.admin_notes = n;
    }

    match state.storage.update_bug_report(&report).await {
        Ok(_) => {
            audit(
                &state,
                claims.user_id,
                "bug_update",
                &format!("Edited bug report #{id}"),
            )
            .await;
            Json(serde_json::json!({"ok": true})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// DELETE /api/v1/bug-reports/:id — delete a report (admin).
pub async fn delete_bug_report(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.storage.delete_bug_report(id).await {
        Ok(_) => {
            audit(
                &state,
                claims.user_id,
                "bug_delete",
                &format!("Deleted bug report #{id}"),
            )
            .await;
            Json(serde_json::json!({"ok": true})).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct ApproveBody {
    pub model: Option<String>,
}

/// POST /api/v1/bug-reports/:id/approve — approve and launch an AI fix job (admin).
pub async fn approve_bug_report(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<ApproveBody>,
) -> impl IntoResponse {
    if !state.config.aibug.enabled {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "AI bug-fix runner is disabled (set [aibug] enabled = true on the master/build server)"})),
        )
            .into_response();
    }
    if !state.is_master() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(
                serde_json::json!({"error": "AI bug-fix runner is only available in master mode"}),
            ),
        )
            .into_response();
    }

    let report = match state.storage.get_bug_report(id).await {
        Ok(r) => r,
        Err(crate::storage::StorageError::NotFound) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    // Serialize jobs: refuse if one is already active.
    let active = state.bug_job_handles.read().await.len();
    if active > 0 {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "Another AI fix job is already running. Try again when it finishes."})),
        )
            .into_response();
    }

    let model = body
        .model
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| state.config.aibug.default_model.clone());

    let job = BugJob {
        id: 0,
        bug_report_id: id,
        model: model.clone(),
        status: "queued".to_string(),
        branch_name: format!("ai/bug-{id}"),
        git_commit: None,
        log: String::new(),
        error: None,
        created_by: Some(claims.user_id),
        started_at: None,
        finished_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let job_id = match state.storage.create_bug_job(&job).await {
        Ok(jid) => jid,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    // Mark report approved and spawn the runner.
    let mut updated = report.clone();
    updated.status = "approved".to_string();
    let _ = state.storage.update_bug_report(&updated).await;

    audit(
        &state,
        claims.user_id,
        "bug_approve",
        &format!("Approved bug #{id}; launched AI fix job #{job_id} (model {model})"),
    )
    .await;

    crate::aibug::spawn_job(state.clone(), job_id, report, model);

    Json(serde_json::json!({"job_id": job_id, "status": "queued"})).into_response()
}

// ---- Jobs ----

/// GET /api/v1/bug-jobs — list jobs, optionally by report (admin).
pub async fn list_bug_jobs(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(50).min(200);
    let offset = q.offset.unwrap_or(0);
    let jobs = state
        .storage
        .list_bug_jobs(None, limit, offset)
        .await
        .unwrap_or_default();
    Json(serde_json::json!({ "jobs": jobs }))
}

/// GET /api/v1/bug-jobs/:id — single job (status + log) for polling (admin).
pub async fn get_bug_job(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.storage.get_bug_job(id).await {
        Ok(job) => Json(serde_json::json!({ "job": job })).into_response(),
        Err(crate::storage::StorageError::NotFound) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// POST /api/v1/bug-jobs/:id/cancel — abort a running job (admin).
pub async fn cancel_bug_job(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let removed = {
        let mut map = state.bug_job_handles.write().await;
        map.remove(&id)
    };
    if let Some(handle) = removed {
        handle.abort();
        let _ = state
            .storage
            .set_bug_job_status(id, "cancelled", Some("Cancelled by admin"))
            .await;
        if let Ok(job) = state.storage.get_bug_job(id).await {
            let mut r = state.storage.get_bug_report(job.bug_report_id).await.ok();
            if let Some(report) = r.as_mut() {
                report.status = "triaged".to_string();
                let _ = state.storage.update_bug_report(report).await;
            }
        }
        audit(
            &state,
            claims.user_id,
            "bug_job_cancel",
            &format!("Cancelled AI fix job #{id}"),
        )
        .await;
        Json(serde_json::json!({"ok": true})).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "No running job with that id"})),
        )
            .into_response()
    }
}

/// GET /api/v1/ai/models — list selectable models (admin).
pub async fn list_models(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let models = crate::aibug::list_models(&state.config.aibug).await;
    Json(serde_json::json!({
        "models": models,
        "default": state.config.aibug.default_model,
    }))
}

// ---- helpers ----

async fn audit(state: &AppState, admin_user_id: i64, action: &str, detail: &str) {
    let _ = state
        .storage
        .save_audit_entry(&AuditEntry {
            id: 0,
            admin_user_id: Some(admin_user_id),
            action: action.to_string(),
            detail: detail.to_string(),
            ip_address: None,
            created_at: chrono::Utc::now(),
            server_id: None,
        })
        .await;
}
