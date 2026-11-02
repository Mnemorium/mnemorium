CREATE TABLE music_album (
    music_album_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    release_date DATE,
    production_type VARCHAR(20),
    cover_image_id INTEGER,
    music_group_id INTEGER NOT NULL,
    CONSTRAINT pk_music_album_music_album_id PRIMARY KEY (music_album_id),
    CONSTRAINT uq_music_album_name_music_group_id UNIQUE (
        name,
        music_group_id
    ),
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
    CONSTRAINT fk_music_album_cover_image FOREIGN KEY (
        cover_image_id
    ) REFERENCES image (image_id),
    CONSTRAINT fk_music_album_music_group FOREIGN KEY (
        music_group_id
    ) REFERENCES music_group (music_group_id)
);
