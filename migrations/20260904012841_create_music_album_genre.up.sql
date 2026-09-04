CREATE TABLE music_album_genre (
    music_album_id INTEGER NOT NULL,
    genre_id TEXT NOT NULL,
    CONSTRAINT pk_music_album_genre_album_genre PRIMARY KEY (
        music_album_id, genre_id
    ),
    CONSTRAINT fk_music_album_genre_album FOREIGN KEY (
        music_album_id
    ) REFERENCES music_album (music_album_id),
    CONSTRAINT fk_music_album_genre_genre FOREIGN KEY (
        genre_id
    ) REFERENCES genre (genre_id)
);
