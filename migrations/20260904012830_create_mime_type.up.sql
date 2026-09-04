CREATE TABLE mime_type (
    mime_type_id TEXT NOT NULL,
    type TEXT NOT NULL,
    CONSTRAINT pk_mime_type_mime_type_id PRIMARY KEY (mime_type_id),
    CONSTRAINT chk_mime_type_type CHECK (type IN ('AUDIO', 'VIDEO', 'IMAGE'))
);
