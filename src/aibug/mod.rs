//! AI bug-fix runner.
//!
//! On admin approval, an approved [`crate::core::BugReport`] spawns a job that
//! drives the GitHub Copilot CLI (Claude models, authenticated via Copilot on
//! the build server) to fix the issue inside an isolated git worktree, runs the
//! standard test/build gates, pushes the fix to a branch on GitHub, and
//! publishes the resulting binary to the `dev` update channel for manual
//! promotion.
//!
//! By design this is **Unix + master-mode only** — it runs on the build server
//! (`/opt/r3-build`) which has the Rust toolchain, Node, GitHub write access and
//! the publish directory. On other platforms the runner refuses to start.

#[cfg(unix)]
use std::process::Stdio;

use crate::config::AiBugSection;
use crate::core::BugReport;
use crate::web::state::AppState;

/// Enumerate the models the admin may choose from.
///
/// The GitHub Copilot CLI has no model-enumeration command — models are
/// selected with `--model <id>` and discovered interactively. So the
/// selectable set is the curated list maintained in config (`fallback_models`),
/// which the admin can edit. The configured `default_model` is guaranteed to be
/// present.
pub async fn list_models(cfg: &AiBugSection) -> Vec<String> {
    let mut models = cfg.fallback_models.clone();
    if !cfg.default_model.is_empty() && !models.iter().any(|m| m == &cfg.default_model) {
        models.insert(0, cfg.default_model.clone());
    }
    models
}

/// Spawn the fix job as a background task and register its abort handle so it
/// can be cancelled. The job updates its own status/log in the database.
pub fn spawn_job(state: AppState, job_id: i64, report: BugReport, model: String) {
    let handles = state.bug_job_handles.clone();
    let task = tokio::spawn(async move {
        run_job(state.clone(), job_id, report, model).await;
        // Drop our own handle from the registry on completion.
        state.bug_job_handles.write().await.remove(&job_id);
    });
    let abort = task.abort_handle();
    tokio::spawn(async move {
        handles.write().await.insert(job_id, abort);
    });
}

/// Run a single fix job end to end. All failures are recorded on the job row;
/// this function never panics the caller.
pub async fn run_job(state: AppState, job_id: i64, report: BugReport, model: String) {
    let cfg = state.config.aibug.clone();
    let timeout = std::time::Duration::from_secs(cfg.job_timeout_secs.max(60));

    let result = tokio::time::timeout(
        timeout,
        run_job_inner(&state, job_id, &report, &model, &cfg),
    )
    .await;

    match result {
        Ok(Ok(())) => {
            mark_report_status(&state, report.id, "completed").await;
            let _ = state
                .storage
                .set_bug_job_status(job_id, "success", None)
                .await;
        }
        Ok(Err(e)) => {
            append(&state, job_id, &format!("\n[error] {e}\n")).await;
            mark_report_status(&state, report.id, "failed").await;
            let _ = state
                .storage
                .set_bug_job_status(job_id, "failed", Some(&e))
                .await;
        }
        Err(_) => {
            append(&state, job_id, "\n[error] job timed out\n").await;
            mark_report_status(&state, report.id, "failed").await;
            let _ = state
                .storage
                .set_bug_job_status(job_id, "failed", Some("timed out"))
                .await;
        }
    }
}

#[cfg(unix)]
async fn run_job_inner(
    state: &AppState,
    job_id: i64,
    report: &BugReport,
    model: &str,
    cfg: &AiBugSection,
) -> Result<(), String> {
    use std::path::Path;

    let branch = format!("ai/bug-{}", report.id);
    let worktree = format!("{}/job-{}", cfg.work_dir.trim_end_matches('/'), job_id);

    // Persist the branch name on the job up front.
    if let Ok(mut job) = state.storage.get_bug_job(job_id).await {
        job.branch_name = branch.clone();
        job.started_at = Some(chrono::Utc::now());
        job.status = "running".to_string();
        let _ = state.storage.update_bug_job(&job).await;
    }
    mark_report_status(state, report.id, "in_progress").await;

    append(
        state,
        job_id,
        &format!("=== AI fix job #{job_id} for bug #{} ===\n", report.id),
    )
    .await;
    append(
        state,
        job_id,
        &format!("model: {model}\nbranch: {branch}\nworktree: {worktree}\n\n"),
    )
    .await;

    // Ensure the work_dir exists.
    std::fs::create_dir_all(&cfg.work_dir)
        .map_err(|e| format!("create work_dir {}: {e}", cfg.work_dir))?;

    // Fresh checkout of origin/main in a dedicated worktree.
    append(state, job_id, "--- preparing git worktree ---\n").await;
    run_streamed(state, job_id, &cfg.repo_dir, "git", &["fetch", "origin"]).await?;
    // Remove a stale worktree at this path if present (best effort).
    if Path::new(&worktree).exists() {
        let _ = run_streamed(
            state,
            job_id,
            &cfg.repo_dir,
            "git",
            &["worktree", "remove", "--force", &worktree],
        )
        .await;
    }
    run_streamed(
        state,
        job_id,
        &cfg.repo_dir,
        "git",
        &["worktree", "add", "-B", &branch, &worktree, "origin/main"],
    )
    .await?;

    // Run the agent.
    append(state, job_id, "\n--- running Copilot agent ---\n").await;
    let prompt = build_prompt(report);
    run_streamed(
        state,
        job_id,
        &worktree,
        &cfg.copilot_bin,
        &[
            "--model",
            model,
            // Non-interactive automation: allow tools + file paths without
            // prompts, but explicitly DENY `git push` so the agent can't push
            // on its own — the runner performs the push deterministically after
            // the test gates pass.
            "--allow-all-tools",
            "--allow-all-paths",
            "--deny-tool=shell(git push)",
            "--no-color",
            "-p",
            &prompt,
        ],
    )
    .await
    .map_err(|e| format!("agent failed: {e}"))?;

    // Verify the agent actually changed something. The agent may EITHER leave
    // edits uncommitted in the working tree OR commit them itself (Copilot
    // often does the latter). Treat both as "has changes": a dirty tree, or
    // the branch HEAD having moved ahead of origin/main.
    let dirty = run_capture(&worktree, "git", &["status", "--porcelain"]).await?;
    let ahead = run_capture(
        &worktree,
        "git",
        &["rev-list", "--count", "origin/main..HEAD"],
    )
    .await
    .unwrap_or_default();
    let ahead_n: u32 = ahead.trim().parse().unwrap_or(0);
    if dirty.trim().is_empty() && ahead_n == 0 {
        return Err("agent produced no changes".to_string());
    }

    // Test/build gates.
    let _ = state
        .storage
        .set_bug_job_status(job_id, "testing", None)
        .await;
    // IMPORTANT: build the SvelteKit UI FIRST. The backend embeds `ui/build`
    // via rust-embed (`UiAssets` in web/mod.rs); if that directory is absent,
    // the derived `UiAssets::get` method doesn't exist and EVERY cargo command
    // (clippy/test/build) fails to compile. A fresh worktree has no ui/build,
    // so the UI must be produced before any cargo gate runs.
    append(state, job_id, "\n--- gates: ui build ---\n").await;
    run_streamed(
        state,
        job_id,
        &format!("{worktree}/ui"),
        "npm",
        &["ci", "--loglevel=error"],
    )
    .await?;
    run_streamed(
        state,
        job_id,
        &format!("{worktree}/ui"),
        "npm",
        &["run", "build"],
    )
    .await?;
    append(state, job_id, "\n--- gates: cargo fmt ---\n").await;
    run_streamed(state, job_id, &worktree, "cargo", &["fmt", "--all"]).await?;
    append(state, job_id, "\n--- gates: cargo clippy ---\n").await;
    run_streamed(state, job_id, &worktree, "cargo", &["clippy"]).await?;
    append(state, job_id, "\n--- gates: cargo test ---\n").await;
    run_streamed(state, job_id, &worktree, "cargo", &["test"]).await?;
    append(state, job_id, "\n--- gates: cargo build --release ---\n").await;
    run_streamed(state, job_id, &worktree, "cargo", &["build", "--release"]).await?;

    // Commit & push the branch. The agent may have already committed its edits,
    // and the `cargo fmt` gate above may have re-dirtied the tree. Stage
    // everything and commit ONLY if there's something staged — `git commit`
    // hard-errors on an empty commit, which is not a failure here (the agent's
    // own commit already carries the fix).
    append(state, job_id, "\n--- commit & push ---\n").await;
    run_streamed(state, job_id, &worktree, "git", &["add", "-A"]).await?;
    let staged = run_capture(&worktree, "git", &["diff", "--cached", "--name-only"]).await?;
    if !staged.trim().is_empty() {
        let commit_msg = format!("ai-fix: bug #{} — {}", report.id, report.title);
        run_streamed(
            state,
            job_id,
            &worktree,
            "git",
            &["commit", "-m", &commit_msg],
        )
        .await?;
    } else {
        append(
            state,
            job_id,
            "(nothing left to stage — using the agent's own commit)\n",
        )
        .await;
    }
    let commit = run_capture(&worktree, "git", &["rev-parse", "--short=8", "HEAD"]).await?;
    let commit = commit.trim().to_string();
    run_streamed(
        state,
        job_id,
        &worktree,
        "git",
        &["push", "-u", "origin", &branch, "--force"],
    )
    .await?;

    // Record the commit on the job.
    if let Ok(mut job) = state.storage.get_bug_job(job_id).await {
        job.git_commit = Some(commit.clone());
        let _ = state.storage.update_bug_job(&job).await;
    }

    // Publish the branch build to the dev channel.
    let _ = state
        .storage
        .set_bug_job_status(job_id, "deploying", None)
        .await;
    append(state, job_id, "\n--- publishing to dev channel ---\n").await;
    let publish_script = format!(
        "{}/build-and-publish-dev.sh",
        cfg.repo_dir.trim_end_matches('/')
    );
    if Path::new(&publish_script).exists() {
        run_streamed(
            state,
            job_id,
            &worktree,
            "bash",
            &[&publish_script, "--dir", &worktree, "--branch", &branch],
        )
        .await?;
    } else {
        append(
            state,
            job_id,
            "[warn] build-and-publish-dev.sh not found — skipping publish. Branch pushed; promote manually.\n",
        )
        .await;
    }

    // Best-effort worktree cleanup (the branch lives on origin).
    let _ = run_streamed(
        state,
        job_id,
        &cfg.repo_dir,
        "git",
        &["worktree", "remove", "--force", &worktree],
    )
    .await;

    append(
        state,
        job_id,
        &format!("\n=== done: pushed {branch} @ {commit}, published to dev ===\n"),
    )
    .await;
    Ok(())
}

#[cfg(not(unix))]
async fn run_job_inner(
    _state: &AppState,
    _job_id: i64,
    _report: &BugReport,
    _model: &str,
    _cfg: &AiBugSection,
) -> Result<(), String> {
    Err("AI bug-fix runner is only supported on the Unix build server".to_string())
}

/// Construct the agent prompt from the bug report plus repo conventions.
#[cfg(unix)]
fn build_prompt(report: &BugReport) -> String {
    format!(
        "You are fixing an issue in the Rusty-Rules-Referee codebase (Rust + Axum backend, \
SvelteKit UI under ui/). Make the minimal correct change to resolve the report below, \
following existing conventions. Do not change unrelated code. Ensure `cargo fmt`, \
`cargo test`, and `cd ui && npm run build` all pass, and do not introduce new clippy warnings.\n\n\
# Bug report #{id}\n\
Title: {title}\n\
Severity: {severity}\n\n\
## Description\n{desc}\n\n\
## Steps to reproduce\n{steps}\n",
        id = report.id,
        title = report.title,
        severity = report.severity,
        desc = if report.description.is_empty() {
            "(none)"
        } else {
            &report.description
        },
        steps = if report.steps.is_empty() {
            "(none)"
        } else {
            &report.steps
        },
    )
}

/// Append a chunk to the job log (best effort; logs but never fails the job).
async fn append(state: &AppState, job_id: i64, chunk: &str) {
    let _ = state.storage.append_bug_job_log(job_id, chunk).await;
}

async fn mark_report_status(state: &AppState, report_id: i64, status: &str) {
    if let Ok(mut r) = state.storage.get_bug_report(report_id).await {
        r.status = status.to_string();
        let _ = state.storage.update_bug_report(&r).await;
    }
}

/// Run a command, streaming combined stdout+stderr into the job log. Returns an
/// error if the process exits non-zero or fails to spawn.
#[cfg(unix)]
async fn run_streamed(
    state: &AppState,
    job_id: i64,
    cwd: &str,
    program: &str,
    args: &[&str],
) -> Result<(), String> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::process::Command;

    append(state, job_id, &format!("$ {program} {}\n", args.join(" "))).await;

    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn {program}: {e}"))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    if let Some(out) = stdout {
        let mut reader = BufReader::new(out).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            append(state, job_id, &format!("{line}\n")).await;
        }
    }
    if let Some(err) = stderr {
        let mut reader = BufReader::new(err).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            append(state, job_id, &format!("{line}\n")).await;
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("wait {program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited with {status}"))
    }
}

/// Run a command and capture stdout (trimmed). Used for short queries like
/// `git rev-parse`. Errors on non-zero exit.
#[cfg(unix)]
async fn run_capture(cwd: &str, program: &str, args: &[&str]) -> Result<String, String> {
    use tokio::process::Command;

    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("spawn {program}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
