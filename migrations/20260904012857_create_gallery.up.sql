CREATE TABLE gallery (
    gallery_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    last_modified_at TEXT NOT NULL,
    is_public BOOLEAN NOT NULL,
    CONSTRAINT pk_gallery_gallery_id PRIMARY KEY (gallery_id),
    CONSTRAINT chk_gallery_is_public CHECK (is_public IN (0, 1))
);
