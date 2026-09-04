CREATE TABLE music_group (
    music_group_id INTEGER NOT NULL,
    stage_name VARCHAR(50) NOT NULL,
    CONSTRAINT pk_music_group_music_group_id PRIMARY KEY (music_group_id),
    CONSTRAINT uq_music_group_stage_name UNIQUE (stage_name)
);
