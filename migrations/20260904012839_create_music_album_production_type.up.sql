CREATE TABLE music_album_production_type (
    music_album_production_type_id TEXT NOT NULL,
    music_album_id INTEGER NOT NULL,
    CONSTRAINT pk_music_album_production_type_id_album PRIMARY KEY (
        music_album_production_type_id, music_album_id
    ),
    CONSTRAINT fk_music_album_production_type_album FOREIGN KEY (
        music_album_id
    ) REFERENCES music_album (music_album_id),
    CONSTRAINT chk_music_album_production_type_type CHECK (
        music_album_production_type_id IN (
            'COMPILATION',
            'DJMIX',
            'DEMO',
            'LIVE',
            'MIXTAPE',
            'REMIX',
            'SOUNDTRACK',
            'STUDIO'
        )
    )
);
