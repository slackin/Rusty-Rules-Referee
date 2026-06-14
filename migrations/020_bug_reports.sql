-- 020_bug_reports.sql: Public bug-report intake + AI auto-fix jobs.
--
-- `bug_reports` queues issues submitted from the public (unauthenticated)
-- report page. Admins triage/edit them in the dashboard and, on approval,
-- spawn a `bug_jobs` row that runs the GitHub Copilot CLI on the build
-- server to fix the issue on a branch, test it, push to GitHub, and publish
-- to the `dev` update channel.

-- Public-submitted bug/feature reports (the queue).
CREATE TABLE IF NOT EXISTS bug_reports (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    title          TEXT    NOT NULL,
    description    TEXT    NOT NULL DEFAULT '',
    steps          TEXT    NOT NULL DEFAULT '',
    severity       TEXT    NOT NULL DEFAULT 'normal',  -- low, normal, high, critical
    reporter_email TEXT,                               -- optional contact
    status         TEXT    NOT NULL DEFAULT 'new',     -- new, triaged, approved, in_progress, completed, failed, rejected
    ip_address     TEXT,                               -- submitter IP (for rate limiting / abuse)
    admin_notes    TEXT    NOT NULL DEFAULT '',
    created_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_bug_reports_status ON bug_reports(status);
CREATE INDEX IF NOT EXISTS idx_bug_reports_created ON bug_reports(created_at);

-- AI fix jobs: one row per approved attempt to fix a bug report.
CREATE TABLE IF NOT EXISTS bug_jobs (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    bug_report_id  INTEGER NOT NULL REFERENCES bug_reports(id) ON DELETE CASCADE,
    model          TEXT    NOT NULL DEFAULT '',        -- Copilot model id used for this run
    status         TEXT    NOT NULL DEFAULT 'queued',  -- queued, running, testing, deploying, success, failed, cancelled
    branch_name    TEXT    NOT NULL DEFAULT '',
    git_commit     TEXT,
    log            TEXT    NOT NULL DEFAULT '',         -- streamed agent + build output
    error          TEXT,
    created_by     INTEGER,                            -- admin_users.id that approved
    started_at     TEXT,
    finished_at    TEXT,
    created_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_bug_jobs_report ON bug_jobs(bug_report_id);
CREATE INDEX IF NOT EXISTS idx_bug_jobs_status ON bug_jobs(status);
