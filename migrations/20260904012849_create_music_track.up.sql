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
    CONSTRAINT chk_music_track_track_index CHECK (track_index >= 0),
    CONSTRAINT chk_music_track_one_container CHECK (
        (music_medium_id IS NOT NULL) <> (music_playlist_id IS NOT NULL)
    ),
    CONSTRAINT chk_music_track_medium_track_index CHECK (
        music_medium_id IS NULL OR track_index BETWEEN 0 AND 255
    )
);

CREATE UNIQUE INDEX uq_music_track_medium_track_index
ON music_track (music_medium_id, track_index)
WHERE music_medium_id IS NOT NULL;

CREATE UNIQUE INDEX uq_music_track_playlist_track_index
ON music_track (music_playlist_id, track_index)
WHERE music_playlist_id IS NOT NULL;
