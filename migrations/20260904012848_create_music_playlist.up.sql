CREATE TABLE music_playlist (
    music_playlist_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    user_id INTEGER NOT NULL,
    is_public BOOLEAN NOT NULL DEFAULT 1,
    CONSTRAINT pk_music_playlist_music_playlist_id PRIMARY KEY (
        music_playlist_id
    ),
    CONSTRAINT fk_music_playlist_user FOREIGN KEY (
        user_id
    ) REFERENCES app_user (user_id),
    CONSTRAINT chk_music_playlist_is_public CHECK (is_public IN (0, 1))
);
