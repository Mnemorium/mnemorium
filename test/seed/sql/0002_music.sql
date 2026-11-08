-- Seeded music library: one group, one album with one medium and track, and
-- three playlists owned by the demo account `alice` (user 9001).
--
-- Image 9001 becomes LIBRARY through the group cover; image 9003 becomes USER
-- through the first playlist's cover. The second playlist has no cover, because
-- a USER image belongs to exactly one playlist.
--
-- The second playlist gives the fixture API run a different target for its
-- track delete, so the read it also performs keeps its track. The third,
-- empty playlist is the delete-playlist target.

INSERT INTO music_group (music_group_id, stage_name, cover_image_id)
VALUES (9001, 'Seed Band', 9001);

INSERT INTO music_group_person (music_group_id, person_id)
VALUES (9001, 9001), (9001, 9002);

INSERT INTO music_album (
    music_album_id,
    name,
    release_date,
    production_type,
    cover_image_id,
    music_group_id
)
VALUES (9001, 'Seed Album', '2020-01-01', 'STUDIO', 9001, 9001);

INSERT INTO music_album_group (music_album_id, music_group_id) VALUES (
    9001, 9001
);

INSERT INTO music_album_genre (music_album_id, genre_id) VALUES (9001, 'ROCK');

INSERT INTO music_medium (music_medium_id, music_album_id, type, medium_index)
VALUES (9001, 9001, 'DIGITAL', 1);

INSERT INTO music_recording (music_recording_id, name, isrc_code, audio_id)
VALUES (9001, 'Seed Song', NULL, 9001);

INSERT INTO music_recording_group (music_recording_id, music_group_id)
VALUES (9001, 9001);

INSERT INTO music_recording_genre (music_recording_id, genre_id) VALUES (
    9001, 'ROCK'
);

INSERT INTO music_track (
    music_track_id,
    track_index,
    music_playlist_id,
    music_medium_id,
    music_recording_id
)
VALUES (9001, 1, NULL, 9001, 9001);

INSERT INTO music_playlist (
    music_playlist_id,
    name,
    created_at,
    last_modified_at,
    user_id,
    is_public,
    cover_image_id
)
VALUES (
    9001, 'Seed Playlist', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 9001, 1, 9003
);

INSERT INTO music_track (
    music_track_id,
    track_index,
    music_playlist_id,
    music_medium_id,
    music_recording_id
)
VALUES (9002, 1, 9001, NULL, 9001);

INSERT INTO music_playlist (
    music_playlist_id,
    name,
    created_at,
    last_modified_at,
    user_id,
    is_public,
    cover_image_id
)
VALUES (
    9002,
    'Seed Playlist Two',
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP,
    9001,
    1,
    NULL
);

INSERT INTO music_track (
    music_track_id,
    track_index,
    music_playlist_id,
    music_medium_id,
    music_recording_id
)
VALUES (9003, 1, 9002, NULL, 9001);

INSERT INTO music_playlist (
    music_playlist_id,
    name,
    created_at,
    last_modified_at,
    user_id,
    is_public,
    cover_image_id
)
VALUES (
    9003,
    'Seed Playlist Three',
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP,
    9001,
    1,
    NULL
);
