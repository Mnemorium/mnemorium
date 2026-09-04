CREATE TABLE music_medium (
    music_medium_id INTEGER NOT NULL,
    music_album_id INTEGER NOT NULL,
    type TEXT NOT NULL,
    CONSTRAINT pk_music_medium_music_medium_id PRIMARY KEY (music_medium_id),
    CONSTRAINT uq_music_medium_music_album_id UNIQUE (music_album_id),
    CONSTRAINT fk_music_medium_music_album FOREIGN KEY (
        music_album_id
    ) REFERENCES music_album (music_album_id)
);
