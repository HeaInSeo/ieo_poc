-- ieo coordinator DB schema v0 (02-03). PostgreSQL 17.
-- Pre-release: `CREATE ... IF NOT EXISTS` only creates missing objects and does not alter
-- existing tables, so a dev DB created from an earlier commit of this PR must be recreated.
-- Every write goes through one transaction per request; a non-2xx answer rolls it back,
-- so a refused request leaves 0 rows behind (OpenAPI: "Every non-2xx response leaves
-- server state unchanged").

CREATE TABLE IF NOT EXISTS schema_meta (
    k text PRIMARY KEY,
    v text NOT NULL
);

CREATE OR REPLACE FUNCTION iso(t timestamptz) RETURNS text
    LANGUAGE sql IMMUTABLE AS
$$ SELECT to_char(t AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"') $$;

-- Registered SprintSpec v0 revisions. Immutable once stored, except that the stored
-- mode only widens (draft → executable, 01-07) and the semantic review is recorded once.
CREATE TABLE IF NOT EXISTS sprint_revisions (
    sprint_id          text        NOT NULL,
    revision           bigint      NOT NULL CHECK (revision >= 1),
    repo_id            text        NOT NULL,
    digest             text        NOT NULL CHECK (digest ~ '^[a-f0-9]{64}$'),
    spec               jsonb       NOT NULL,
    mode               text        NOT NULL CHECK (mode IN ('draft', 'executable')),
    registration_state text        NOT NULL CHECK (registration_state IN ('draft', 'ready')),
    review_state       text        NOT NULL DEFAULT 'pending_semantic_review'
                       CHECK (review_state IN ('pending_semantic_review', 'semantic_review_passed', 'semantic_review_rejected')),
    registered_by      text        NOT NULL,
    registered_at      timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (sprint_id, revision)
);

-- Semantic review (P01) is separate from the format/reference check that registration runs.
-- One decision per revision; the reviewer must differ from the registrant (policy G3).
CREATE TABLE IF NOT EXISTS semantic_reviews (
    sprint_id   text        NOT NULL,
    revision    bigint      NOT NULL,
    digest      text        NOT NULL,
    decision    text        NOT NULL CHECK (decision IN ('passed', 'rejected')),
    reason      text        NOT NULL,
    reviewer    text        NOT NULL,
    reviewed_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (sprint_id, revision),
    FOREIGN KEY (sprint_id, revision) REFERENCES sprint_revisions (sprint_id, revision)
);

-- Stored answers of idempotent client requests, scoped per principal and endpoint.
CREATE TABLE IF NOT EXISTS idempotency (
    principal   text        NOT NULL,
    endpoint    text        NOT NULL,
    key         text        NOT NULL,
    body_digest text        NOT NULL,
    mode        text        NOT NULL,
    response    jsonb       NOT NULL,
    stored_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (principal, endpoint, key)
);

-- Every (assignment, generation) the coordinator issued. Append-only history: rows are
-- never updated or deleted. `target_sha` is the coordinator's record (never an agent body).
CREATE TABLE IF NOT EXISTS assignments (
    assignment_id   text        NOT NULL,
    generation      bigint      NOT NULL CHECK (generation >= 0),
    repo_id         text        NOT NULL,
    role            text        NOT NULL CHECK (role IN ('development', 'guardrail')),
    sprint_id       text        NOT NULL,
    sprint_revision bigint      NOT NULL,
    task_id         text        NOT NULL,
    target_sha      text        NOT NULL CHECK (target_sha ~ '^[a-f0-9]{40,64}$'),
    issued_by       text        NOT NULL,
    policy_digest   text        NOT NULL,
    request         jsonb       NOT NULL,
    issued_at       timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (assignment_id, generation),
    FOREIGN KEY (sprint_id, sprint_revision) REFERENCES sprint_revisions (sprint_id, revision)
);

-- The one current (assignment, generation) per (repo, role): the ownership/permission head.
-- Changed only by compare-and-swap on the previous head under a row lock.
CREATE TABLE IF NOT EXISTS assignment_heads (
    repo_id       text   NOT NULL,
    role          text   NOT NULL,
    assignment_id text   NOT NULL,
    generation    bigint NOT NULL,
    PRIMARY KEY (repo_id, role),
    FOREIGN KEY (assignment_id, generation) REFERENCES assignments (assignment_id, generation)
);

-- Cumulative attempts per (sprint, task, role) across assignments and generations (policy G4/G5).
CREATE TABLE IF NOT EXISTS task_attempts (
    sprint_id     text    NOT NULL,
    task_id       text    NOT NULL,
    role          text    NOT NULL,
    issued        bigint  NOT NULL DEFAULT 0,
    limit_reached boolean NOT NULL DEFAULT false,
    -- ceiling at which the one decision request was recorded (a raised ceiling asks again)
    reached_ceiling bigint,
    PRIMARY KEY (sprint_id, task_id, role)
);

-- Progress report v0 events (claims and verified reports), append-only. `sprint_seq` is the
-- commit order within a sprint: inserts take a per-sprint advisory lock held to commit, so a
-- later sequence number never becomes visible before an earlier one (cursor safety).
CREATE TABLE IF NOT EXISTS events (
    sprint_id      text        NOT NULL,
    sprint_seq     bigint      NOT NULL,
    scope_repo     text        NOT NULL,
    scope_role     text        NOT NULL CHECK (scope_role IN ('development', 'guardrail', 'coordinator')),
    event_id       text        NOT NULL,
    event_revision bigint      NOT NULL,
    task_id        text,
    report_type    text        NOT NULL,
    evidence_level text        NOT NULL CHECK (evidence_level IN ('claim', 'verified')),
    disposition    text        CHECK (disposition IN ('current', 'preserved_late')),
    assignment_id  text,
    generation     bigint,
    attributed_sha text,
    policy_digest  text,
    report         jsonb       NOT NULL,
    stored_at      timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (sprint_id, sprint_seq),
    UNIQUE (scope_repo, scope_role, event_id, event_revision),
    -- a claim is stored only with its disposition and the coordinator's SHA attribution
    CHECK (evidence_level = 'verified' OR (disposition IS NOT NULL AND attributed_sha IS NOT NULL))
);

-- Guardrail reproduction requests (01-02 §2.1). The runner itself is 02-06.
CREATE TABLE IF NOT EXISTS repro_requests (
    repo_id    text        NOT NULL,
    request_id text        NOT NULL,
    body       jsonb       NOT NULL,
    state      text        NOT NULL CHECK (state IN ('queued', 'running', 'finished', 'runner_failed', 'rejected_stale_generation')),
    target_sha text        NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (repo_id, request_id)
);

-- GitHub and execution evidence ingested by an evidence service. Inputs to verdicts.
CREATE TABLE IF NOT EXISTS evidence (
    repo_id     text        NOT NULL,
    evidence_id text        NOT NULL,
    kind        text        NOT NULL CHECK (kind IN ('ci_run', 'run_record', 'review', 'merge', 'main_ci', 'output', 'followups')),
    sha         text        NOT NULL CHECK (sha ~ '^[a-f0-9]{40,64}$'),
    body        jsonb       NOT NULL,
    ingested_by text        NOT NULL,
    seq         bigserial,
    stored_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (repo_id, evidence_id)
);

-- Verdicts (coordinator_verdict) with the facts and policy digest they were made under.
CREATE TABLE IF NOT EXISTS judgments (
    repo_id       text        NOT NULL,
    judgment_id   text        NOT NULL,
    request       jsonb       NOT NULL,
    sprint_id     text        NOT NULL,
    -- revision whose task definition was judged (a prerequisite counts only while unchanged)
    sprint_revision bigint    NOT NULL,
    task_id       text        NOT NULL,
    outcome       text        NOT NULL CHECK (outcome IN ('accepted', 'not_accepted')),
    head_sha      text        NOT NULL,
    merged_sha    text,
    facts         jsonb       NOT NULL,
    policy_digest text        NOT NULL,
    event_sprint  text        NOT NULL,
    event_seq     bigint      NOT NULL,
    judged_by     text        NOT NULL,
    judged_at     timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (repo_id, judgment_id),
    FOREIGN KEY (sprint_id, sprint_revision) REFERENCES sprint_revisions (sprint_id, revision),
    FOREIGN KEY (event_sprint, event_seq) REFERENCES events (sprint_id, sprint_seq)
);

-- Outbox: written in the same transaction as the verified event it delivers.
CREATE TABLE IF NOT EXISTS outbox (
    outbox_id   bigserial   PRIMARY KEY,
    sprint_id   text        NOT NULL,
    sprint_seq  bigint      NOT NULL,
    destination text        NOT NULL,
    state       text        NOT NULL DEFAULT 'pending' CHECK (state IN ('pending', 'sent', 'unknown')),
    updated_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (sprint_id, sprint_seq, destination),
    FOREIGN KEY (sprint_id, sprint_seq) REFERENCES events (sprint_id, sprint_seq)
);

-- History of every accepted state change (who, what, when). Append-only.
CREATE TABLE IF NOT EXISTS history (
    seq       bigserial   PRIMARY KEY,
    actor     text        NOT NULL,
    action    text        NOT NULL,
    subject   jsonb       NOT NULL,
    at        timestamptz NOT NULL DEFAULT now()
);

-- Notion imports (X-01): one row per (page, content digest). Same content again is a replay;
-- changed content becomes the next revision of the test sprint. Rows are never changed.
CREATE TABLE IF NOT EXISTS notion_imports (
    page_id          text        NOT NULL,
    content_digest   text        NOT NULL CHECK (content_digest ~ '^[a-f0-9]{64}$'),
    test_id          text        NOT NULL,
    url              text        NOT NULL,
    last_edited_time text        NOT NULL,
    source           jsonb       NOT NULL,
    sprint_revision  bigint      NOT NULL,
    response         jsonb       NOT NULL,
    imported_by      text        NOT NULL,
    -- X-01 C2: queue gates and missing facts as the import service recorded them (not spec text);
    -- NULL = imported before C2 (derived fail-closed from the stored response, see notion_gate)
    gates            jsonb,
    missing          jsonb,
    imported_at      timestamptz NOT NULL DEFAULT now(),
    -- one row per import, i.e. per test-sprint revision it created: content that returns to an
    -- earlier digest is a new row (review 9a421b7 F2). Keyed by the sprint revision, which never
    -- conflicts on earlier data where a page was imported under two test ids (review 7afd4d1 G4);
    -- "latest import of a page" is ordered by imported_at.
    PRIMARY KEY (test_id, sprint_revision),
    FOREIGN KEY (test_id, sprint_revision) REFERENCES sprint_revisions (sprint_id, revision)
);

-- C2 migration for databases created before C2 (columns stay NULL on earlier rows)
ALTER TABLE notion_imports ADD COLUMN IF NOT EXISTS gates jsonb;
ALTER TABLE notion_imports ADD COLUMN IF NOT EXISTS missing jsonb;
DO $$ BEGIN
  IF EXISTS (SELECT 1 FROM pg_constraint c JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = ANY (c.conkey)
             WHERE c.conname = 'notion_imports_pkey' AND a.attname = 'content_digest') THEN
    ALTER TABLE notion_imports DROP CONSTRAINT notion_imports_pkey;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'notion_imports_pkey') THEN
    ALTER TABLE notion_imports ADD PRIMARY KEY (test_id, sprint_revision);
  ELSIF EXISTS (SELECT 1 FROM pg_constraint c JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = ANY (c.conkey)
                WHERE c.conname = 'notion_imports_pkey' AND a.attname = 'page_id') THEN
    -- (page_id, sprint_revision) from 7afd4d1
    ALTER TABLE notion_imports DROP CONSTRAINT notion_imports_pkey;
    ALTER TABLE notion_imports ADD PRIMARY KEY (test_id, sprint_revision);
  END IF;
END $$;

-- X-01 C2: user-approved execution exceptions for an imported test sprint. They lift the
-- recorded queue gates only, for exactly one (test_id, repo); never missing facts or other checks.
CREATE TABLE IF NOT EXISTS exec_exceptions (
    exception_id text        PRIMARY KEY,
    test_id      text        NOT NULL,
    repo_id      text        NOT NULL,
    lifts        text        NOT NULL CHECK (lifts = 'queue_gates'),
    scope        jsonb       NOT NULL,
    decision_ref text        NOT NULL,
    request      jsonb       NOT NULL,
    approved_by  text        NOT NULL,
    -- content digest of the import the exception was approved against (review 9a421b7 F4);
    -- a later, different gated import needs a new approval. NULL (earlier rows) never matches.
    content_digest text,
    -- the Notion page of that import (review 7afd4d1 G2: digests do not name a page). NULL never matches.
    page_id      text,
    approved_at  timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE exec_exceptions ADD COLUMN IF NOT EXISTS content_digest text;
ALTER TABLE exec_exceptions ADD COLUMN IF NOT EXISTS page_id text;

-- Append-only history: UPDATE/DELETE on these tables is refused by the database itself.
CREATE OR REPLACE FUNCTION forbid_change() RETURNS trigger LANGUAGE plpgsql AS
$$ BEGIN RAISE EXCEPTION 'append-only table %', TG_TABLE_NAME; END $$;
CREATE OR REPLACE TRIGGER assignments_append_only BEFORE UPDATE OR DELETE ON assignments
    FOR EACH ROW EXECUTE FUNCTION forbid_change();
CREATE OR REPLACE TRIGGER events_append_only BEFORE UPDATE OR DELETE ON events
    FOR EACH ROW EXECUTE FUNCTION forbid_change();
CREATE OR REPLACE TRIGGER evidence_append_only BEFORE UPDATE OR DELETE ON evidence
    FOR EACH ROW EXECUTE FUNCTION forbid_change();
CREATE OR REPLACE TRIGGER judgments_append_only BEFORE UPDATE OR DELETE ON judgments
    FOR EACH ROW EXECUTE FUNCTION forbid_change();
CREATE OR REPLACE TRIGGER semantic_reviews_append_only BEFORE UPDATE OR DELETE ON semantic_reviews
    FOR EACH ROW EXECUTE FUNCTION forbid_change();
CREATE OR REPLACE TRIGGER history_append_only BEFORE UPDATE OR DELETE ON history
    FOR EACH ROW EXECUTE FUNCTION forbid_change();
CREATE OR REPLACE TRIGGER notion_imports_append_only BEFORE UPDATE OR DELETE ON notion_imports
    FOR EACH ROW EXECUTE FUNCTION forbid_change();
CREATE OR REPLACE TRIGGER exec_exceptions_append_only BEFORE UPDATE OR DELETE ON exec_exceptions
    FOR EACH ROW EXECUTE FUNCTION forbid_change();

INSERT INTO schema_meta (k, v) VALUES ('schema_version', 'ieo-coord-v0') ON CONFLICT (k) DO NOTHING;
