CREATE TABLE root_admin_setup (
    root_admin_setup_id INTEGER PRIMARY KEY,
    singleton INTEGER NOT NULL DEFAULT 0,
    nonce TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    consumed_at TEXT,
    CONSTRAINT uq_root_admin_setup_singleton UNIQUE (singleton),
    CONSTRAINT chk_root_admin_setup_singleton CHECK (singleton = 0),
    CONSTRAINT chk_root_admin_setup_nonce CHECK (length(nonce) >= 32)
);
