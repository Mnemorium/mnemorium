CREATE TABLE music_playlist (
    music_playlist_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_modified_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    user_id INTEGER NOT NULL,
    is_public BOOLEAN NOT NULL DEFAULT 0,
    cover_file_id INTEGER,
    CONSTRAINT pk_music_playlist_music_playlist_id PRIMARY KEY (
        music_playlist_id
    ),
    CONSTRAINT fk_music_playlist_user FOREIGN KEY (
        user_id
    ) REFERENCES user (user_id),
    CONSTRAINT fk_music_playlist_cover_file FOREIGN KEY (
        cover_file_id
    ) REFERENCES file (file_id),
    CONSTRAINT chk_music_playlist_is_public CHECK (is_public IN (0, 1)),
    CONSTRAINT uq_music_playlist_user_id_name UNIQUE (user_id, name)
);
