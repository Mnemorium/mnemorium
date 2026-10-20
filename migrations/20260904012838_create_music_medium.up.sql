CREATE TABLE music_medium (
    music_medium_id INTEGER NOT NULL,
    music_album_id INTEGER NOT NULL,
    type TEXT NOT NULL,
    medium_index INTEGER NOT NULL DEFAULT 1,
    CONSTRAINT pk_music_medium_music_medium_id PRIMARY KEY (music_medium_id),
    CONSTRAINT uq_music_medium_album_medium_index UNIQUE (
        music_album_id,
        medium_index
    ),
    CONSTRAINT chk_music_medium_medium_index CHECK (
        medium_index BETWEEN 0 AND 255
    ),
    CONSTRAINT chk_music_medium_type CHECK (type IN ('CD')),
    CONSTRAINT fk_music_medium_music_album FOREIGN KEY (
        music_album_id
    ) REFERENCES music_album (music_album_id)
);
