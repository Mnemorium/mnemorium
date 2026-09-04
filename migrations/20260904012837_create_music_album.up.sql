CREATE TABLE music_album (
    music_album_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    release_date DATE NOT NULL,
    CONSTRAINT pk_music_album_music_album_id PRIMARY KEY (music_album_id)
);
