CREATE TABLE music_track (
    music_track_id INTEGER NOT NULL,
    track_index INTEGER NOT NULL,
    music_playlist_id INTEGER,
    music_medium_id INTEGER,
    music_recording_id INTEGER NOT NULL,
    CONSTRAINT pk_music_track_music_track_id PRIMARY KEY (music_track_id),
    CONSTRAINT fk_music_track_playlist FOREIGN KEY (
        music_playlist_id
    ) REFERENCES music_playlist (music_playlist_id),
    CONSTRAINT fk_music_track_medium FOREIGN KEY (
        music_medium_id
    ) REFERENCES music_medium (music_medium_id),
    CONSTRAINT fk_music_track_recording FOREIGN KEY (
        music_recording_id
    ) REFERENCES music_recording (music_recording_id),
    CONSTRAINT chk_music_track_track_index CHECK (track_index >= 0)
);
