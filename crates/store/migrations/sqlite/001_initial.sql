-- kantui initial schema (SQLite).
-- IDs are stored as lowercase hex-32 TEXT (matches EntityId::Display).
-- Timestamps are stored as INTEGER milliseconds since the Unix epoch.

CREATE TABLE projects (
    id          TEXT    NOT NULL PRIMARY KEY,
    name        TEXT    NOT NULL,
    description TEXT,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE TABLE states (
    id         TEXT    NOT NULL PRIMARY KEY,
    project_id TEXT    NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name       TEXT    NOT NULL,
    position   INTEGER NOT NULL,
    wip_limit  INTEGER
);
CREATE INDEX states_by_project ON states (project_id, position);

CREATE TABLE tasks (
    id          TEXT    NOT NULL PRIMARY KEY,
    project_id  TEXT    NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    state_id    TEXT    NOT NULL REFERENCES states(id)   ON DELETE CASCADE,
    title       TEXT    NOT NULL,
    description TEXT,
    priority    TEXT    NOT NULL,
    complexity  TEXT    NOT NULL,
    due_date    INTEGER,
    position    INTEGER NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
CREATE INDEX tasks_by_state   ON tasks (state_id, position);
CREATE INDEX tasks_by_project ON tasks (project_id);

CREATE TABLE tags (
    id    TEXT NOT NULL PRIMARY KEY,
    name  TEXT NOT NULL UNIQUE,
    color TEXT NOT NULL
);

CREATE TABLE task_tags (
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    tag_id  TEXT NOT NULL REFERENCES tags(id)  ON DELETE CASCADE,
    PRIMARY KEY (task_id, tag_id)
);

-- Append-only log of every state move; powers the sojourn dashboard.
CREATE TABLE task_transitions (
    task_id    TEXT    NOT NULL REFERENCES tasks(id)  ON DELETE CASCADE,
    from_state TEXT    REFERENCES states(id)          ON DELETE SET NULL,
    to_state   TEXT    NOT NULL REFERENCES states(id) ON DELETE CASCADE,
    at         INTEGER NOT NULL,
    PRIMARY KEY (task_id, at)
);
CREATE INDEX task_transitions_by_state ON task_transitions (to_state, at);
