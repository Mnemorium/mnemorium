CREATE TABLE music_recording (
    music_recording_id INTEGER NOT NULL,
    name TEXT,
    first_release_date DATE NOT NULL,
    isrc_code VARCHAR(12),
    audio_id INTEGER NOT NULL,
    CONSTRAINT pk_music_recording_music_recording_id PRIMARY KEY (
        music_recording_id
    ),
    CONSTRAINT uq_music_recording_isrc_code UNIQUE (isrc_code),
    CONSTRAINT fk_music_recording_audio FOREIGN KEY (
        audio_id
    ) REFERENCES audio (audio_id)
);
