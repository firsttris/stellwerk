-- One revision for the whole configuration; every applied change raises it.
CREATE TABLE config_rev (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    revision INTEGER NOT NULL
);
INSERT INTO config_rev (id, revision) VALUES (1, 0);

CREATE TABLE devices (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    os TEXT NOT NULL,
    public_key TEXT NOT NULL UNIQUE,
    syncthing_id TEXT NOT NULL,
    agent_version TEXT NOT NULL,
    syncthing_version TEXT NOT NULL,
    approved INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    last_seen INTEGER,
    applied_rev INTEGER
);

CREATE TABLE folders (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    syncthing_folder_id TEXT NOT NULL UNIQUE,
    ignore_patterns TEXT NOT NULL DEFAULT '',
    versioning TEXT NOT NULL,
    encrypted_on_hub INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE TABLE shares (
    folder_id TEXT NOT NULL REFERENCES folders (id) ON DELETE CASCADE,
    device_id TEXT NOT NULL REFERENCES devices (id) ON DELETE CASCADE,
    mode TEXT NOT NULL,
    path TEXT NOT NULL,
    PRIMARY KEY (folder_id, device_id)
);

CREATE TABLE enrollments (
    code_hash TEXT PRIMARY KEY,
    expires_at INTEGER NOT NULL,
    used_at INTEGER,
    device_id TEXT REFERENCES devices (id) ON DELETE SET NULL
);

CREATE TABLE events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    at INTEGER NOT NULL,
    device_id TEXT,
    folder_id TEXT,
    kind TEXT NOT NULL,
    text TEXT NOT NULL
);

CREATE TABLE admin (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    password_hash TEXT NOT NULL
);

CREATE TABLE sessions (
    token_hash TEXT PRIMARY KEY,
    csrf_token TEXT NOT NULL,
    expires_at INTEGER NOT NULL
);
