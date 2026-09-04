CREATE TABLE music_recording_genre (
    music_recording_id INTEGER NOT NULL,
    genre_id TEXT NOT NULL,
    CONSTRAINT pk_music_recording_genre_recording_genre PRIMARY KEY (
        music_recording_id, genre_id
    ),
    CONSTRAINT fk_music_recording_genre_recording FOREIGN KEY (
        music_recording_id
    ) REFERENCES music_recording (music_recording_id),
    CONSTRAINT fk_music_recording_genre_genre FOREIGN KEY (
        genre_id
    ) REFERENCES genre (genre_id)
);
