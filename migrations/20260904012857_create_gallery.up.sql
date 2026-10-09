CREATE TABLE gallery (
    gallery_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    last_modified_at TEXT NOT NULL,
    is_public BOOLEAN NOT NULL,
    user_id INTEGER,
    CONSTRAINT pk_gallery_gallery_id PRIMARY KEY (gallery_id),
    CONSTRAINT fk_gallery_user FOREIGN KEY (user_id) REFERENCES user (user_id),
    CONSTRAINT chk_gallery_is_public CHECK (is_public IN (0, 1)),
    CONSTRAINT uq_gallery_user_id_name UNIQUE (user_id, name)
);
