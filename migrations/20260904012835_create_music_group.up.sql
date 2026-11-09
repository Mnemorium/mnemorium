CREATE TABLE music_group (
    music_group_id INTEGER NOT NULL,
    stage_name VARCHAR(50) NOT NULL,
    cover_image_id INTEGER,
    CONSTRAINT pk_music_group_music_group_id PRIMARY KEY (music_group_id),
    CONSTRAINT uq_music_group_stage_name UNIQUE (stage_name),
    CONSTRAINT fk_music_group_cover_image FOREIGN KEY (
        cover_image_id
    ) REFERENCES image (image_id) ON DELETE SET NULL
);
