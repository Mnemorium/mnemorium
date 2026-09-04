CREATE TABLE music_recording_group (
    music_recording_id INTEGER NOT NULL,
    music_group_id INTEGER NOT NULL,
    CONSTRAINT pk_music_recording_group_recording_group PRIMARY KEY (
        music_recording_id, music_group_id
    ),
    CONSTRAINT fk_music_recording_group_recording FOREIGN KEY (
        music_recording_id
    ) REFERENCES music_recording (music_recording_id),
    CONSTRAINT fk_music_recording_group_group FOREIGN KEY (
        music_group_id
    ) REFERENCES music_group (music_group_id)
);
