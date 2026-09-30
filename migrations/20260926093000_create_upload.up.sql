CREATE TABLE upload (
    upload_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    file_name TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    mime_type_id TEXT NOT NULL,
    chunk_size INTEGER NOT NULL,
    integrity_hash CHAR(64) NOT NULL,
    chunk_bitmap BLOB NOT NULL,
    is_finished BOOLEAN NOT NULL DEFAULT 0,
    version INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT pk_upload_upload_id PRIMARY KEY (upload_id),
    CONSTRAINT fk_upload_user FOREIGN KEY (user_id) REFERENCES user (user_id),
    CONSTRAINT fk_upload_mime_type FOREIGN KEY (
        mime_type_id
    ) REFERENCES mime_type (mime_type_id),
    CONSTRAINT chk_upload_file_size CHECK (file_size > 0),
    CONSTRAINT chk_upload_chunk_size CHECK (chunk_size > 0),
    CONSTRAINT chk_upload_integrity_hash CHECK (LENGTH(integrity_hash) = 64),
    CONSTRAINT chk_upload_is_finished CHECK (is_finished IN (0, 1)),
    CONSTRAINT chk_upload_version CHECK (version >= 0)
);
