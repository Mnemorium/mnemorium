CREATE TABLE file (
    file_id INTEGER NOT NULL,
    path TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    is_public BOOLEAN NOT NULL DEFAULT 0,
    mime_type_id TEXT NOT NULL,
    uploaded_at DATE NOT NULL DEFAULT (date('now')),
    md5_integrity VARCHAR(128) NOT NULL,
    CONSTRAINT pk_file_file_id PRIMARY KEY (file_id),
    CONSTRAINT uq_file_path UNIQUE (path),
    CONSTRAINT uq_file_md5_integrity UNIQUE (md5_integrity),
    CONSTRAINT fk_file_user FOREIGN KEY (user_id) REFERENCES user (user_id),
    CONSTRAINT fk_file_mime_type FOREIGN KEY (
        mime_type_id
    ) REFERENCES mime_type (mime_type_id),
    CONSTRAINT chk_file_is_public CHECK (is_public IN (0, 1)),
    CONSTRAINT chk_file_md5_integrity CHECK (length(md5_integrity) = 128)
);
