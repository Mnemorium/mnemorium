PRAGMA foreign_keys = ON;

CREATE TABLE credential (
    credential_id INTEGER NOT NULL,
    password_hash TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT pk_credential_credential_id PRIMARY KEY (credential_id),
    CONSTRAINT uq_credential_password_hash UNIQUE (password_hash)
);
