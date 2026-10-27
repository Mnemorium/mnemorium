CREATE TABLE music_album (
    music_album_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    release_date DATE NOT NULL,
    production_type VARCHAR(20),
    cover_file_id INTEGER,
    CONSTRAINT pk_music_album_music_album_id PRIMARY KEY (music_album_id),
    CONSTRAINT chk_music_album_production_type CHECK (
        production_type IN (
            'COMPILATION',
            'DJMIX',
            'DEMO',
            'LIVE',
            'MIXTAPE',
            'REMIX',
            'SOUNDTRACK',
            'STUDIO'
        )
    ),
    CONSTRAINT fk_music_album_cover_file FOREIGN KEY (
        cover_file_id
    ) REFERENCES file (file_id)
);
