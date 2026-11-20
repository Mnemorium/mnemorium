-- The system profile's cover-art fixture (identifier block 11000-11999).
--
-- Every module block shares one internal layout, so a case can name an
-- identifier without reading the datastore:
--   11000-11099  catalogue rows (a group, an album, a medium, a playlist)
--   11100-11199  the module's file rows
--   11200-11299  the module's typed-media rows (images, tracks, episodes)
--
-- The rows arrive with the module's cases, one slot group per case. Every
-- library row is PUBLIC and carries the resource the create use case writes;
-- the playlist rows are PRIVATE and owned by the profile's identity (10000).
-- Each image carries the scope its referrer fixes, so the attachment triggers
-- find nothing to change and no image is attached twice.
--
-- Slots:
--   11001, 11002  a covered and a bare music group
--   11003, 11004  a covered and a bare music album
--   11005        the covered album's medium at index 1
--   11006, 11018  the covered album's two tracks
--   11007, 11017  the recordings those tracks place
--   11008        the private playlist of the playlist-track case
--   11009        its track at index 0
--   11010        the playlist the replace case patches
--   11011        the playlist the clear case patches
--   11012, 11013  the private gallery and its one item
--   11014        the show whose episode carries no artwork
--   11015        its season at index 1
--   11016        its episode at index 0
--   11101-11110   the ten committed PNGs
--   11111, 11112  the two committed FLAC copies
--   11113        the committed episode video
--   11201-11210   the images those PNGs back
--   11211, 11212  the audios the two recordings place
--   11213        the episode's video
--
-- The file rows and their PUBLIC FILE resources are declared below, before the
-- typed rows that reference them. The seed's grounding step copies each
-- committed byte under the library root at the path the row declares and
-- records the size and modification instant a scan compares, so every file is
-- reported untouched. `size_bytes` and `modified_at` are placeholders because
-- SQL cannot know them.
--
-- Blocks: identity 10000, cover art 11000, music 12000, movies 13000, audio
-- 14000, file 15000, download 16000.

INSERT INTO file (
    file_id,
    path,
    user_id,
    is_public,
    mime_type_id,
    size_bytes,
    modified_at,
    origin,
    integrity_hash
)
VALUES
(
    11101, 'system/cover-group.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    11102, 'system/cover-album.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    11103, 'system/cover-series.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    11104, 'system/cover-season.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    11105, 'system/playlist-new.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    11106, 'system/playlist-replaced.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    11107, 'system/playlist-replacement.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    11108, 'system/playlist-cleared.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    11109, 'system/gallery-media.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    11110, 'system/library-cover.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    11111, 'system/track.flac', 10000, 1,
    'audio/flac', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    11112, 'system/track-copy.flac', 10000, 1,
    'audio/flac', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    11113, 'system/episode.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
);

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
VALUES
('FILE', 11101, 10000, NULL, 'PUBLIC'),
('FILE', 11102, 10000, NULL, 'PUBLIC'),
('FILE', 11103, 10000, NULL, 'PUBLIC'),
('FILE', 11104, 10000, NULL, 'PUBLIC'),
('FILE', 11105, 10000, NULL, 'PUBLIC'),
('FILE', 11106, 10000, NULL, 'PUBLIC'),
('FILE', 11107, 10000, NULL, 'PUBLIC'),
('FILE', 11108, 10000, NULL, 'PUBLIC'),
('FILE', 11109, 10000, NULL, 'PUBLIC'),
('FILE', 11110, 10000, NULL, 'PUBLIC'),
('FILE', 11111, 10000, NULL, 'PUBLIC'),
('FILE', 11112, 10000, NULL, 'PUBLIC'),
('FILE', 11113, 10000, NULL, 'PUBLIC')
ON CONFLICT DO NOTHING;

INSERT INTO image (
    image_id,
    name,
    width_px,
    height_px,
    orientation,
    created_at,
    scope,
    file_id
)
VALUES
(11201, 'cover-group.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 11101),
(11202, 'cover-album.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 11102),
(11203, 'cover-series.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 11103),
(11204, 'cover-season.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 11104),
(11205, 'playlist-new.png', 64, 64, 'SQUARE', '2020-01-01', 'USER', 11105),
(11206, 'playlist-replaced.png', 64, 64, 'SQUARE', '2020-01-01', 'USER', 11106),
(
    11207, 'playlist-replacement.png', 64, 64, 'SQUARE', '2020-01-01',
    'USER', 11107
),
(11208, 'playlist-cleared.png', 64, 64, 'SQUARE', '2020-01-01', 'USER', 11108),
(11209, 'gallery-media.png', 64, 64, 'SQUARE', '2020-01-01', 'USER', 11109),
(11210, 'library-cover.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 11110);

INSERT INTO audio (
    audio_id,
    name,
    duration_ms,
    codec,
    sample_rate_hz,
    bit_depth,
    block_size,
    file_id
)
VALUES
(11211, 'track.flac', 500.0, 'FLAC', 44100.0, 16, NULL, 11111),
(11212, 'track-copy.flac', 500.0, 'FLAC', 44100.0, 16, NULL, 11112);

INSERT INTO video (
    video_id,
    duration_ms,
    codec,
    frame_count,
    width,
    height,
    color_id,
    scan_type,
    scope,
    file_id
)
VALUES (
    11213, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 11113
);

INSERT INTO music_group (music_group_id, stage_name, cover_image_id)
VALUES
(11001, 'Covered Group', 11201),
(11002, 'Bare Group', NULL);

INSERT INTO music_album (
    music_album_id,
    name,
    release_date,
    production_type,
    cover_image_id,
    music_group_id
)
VALUES
(11003, 'Covered Album', '2020-01-01', 'STUDIO', 11202, 11001),
(11004, 'Bare Album', NULL, NULL, NULL, 11002);

INSERT INTO music_album_genre (music_album_id, genre_id)
VALUES (11003, 'UNKNOWN'), (11004, 'UNKNOWN');

INSERT INTO music_medium (music_medium_id, music_album_id, type, medium_index)
VALUES (11005, 11003, 'CD', 1);

INSERT INTO music_recording (music_recording_id, name, isrc_code, audio_id)
VALUES
(11007, 'Covered Track', NULL, 11211),
(11017, 'Playlist Track', NULL, 11212);

INSERT INTO music_playlist (
    music_playlist_id,
    name,
    created_at,
    last_modified_at,
    user_id,
    is_public,
    cover_image_id
)
VALUES
(
    11008, 'Playlist Track Case', '2020-01-01T00:00:00',
    '2020-01-01T00:00:00', 10000, 0, NULL
),
(
    11010, 'Replaced Cover', '2020-01-01T00:00:00', '2020-01-01T00:00:00',
    10000, 0, 11206
),
(
    11011, 'Cleared Cover', '2020-01-01T00:00:00', '2020-01-01T00:00:00',
    10000, 0, 11208
);

INSERT INTO music_track (
    music_track_id,
    track_index,
    music_playlist_id,
    music_medium_id,
    music_recording_id
)
VALUES
(11006, 0, NULL, 11005, 11007),
(11018, 1, NULL, 11005, 11017),
(11009, 0, 11008, NULL, 11017);

INSERT INTO gallery (
    gallery_id,
    name,
    created_at,
    last_modified_at,
    is_public,
    user_id
)
VALUES (
    11012, 'Gallery Media Case', '2020-01-01T00:00:00',
    '2020-01-01T00:00:00', 0, 10000
);

INSERT INTO gallery_item (gallery_item_id, gallery_id, image_id, item_index)
VALUES (11013, 11012, 11209, 0);

INSERT INTO series (
    series_id,
    name,
    plot,
    country_of_origin,
    first_air_date,
    cover_image_id
)
VALUES (
    11014, 'Covered Show', 'A show with its own artwork.', 'US', '2020-01-01',
    11203
);

INSERT INTO season (
    season_id,
    series_id,
    season_index,
    name,
    cover_image_id
)
VALUES (11015, 11014, 1, 'Season With Artwork', 11204);

INSERT INTO episode (
    episode_id,
    season_id,
    episode_index,
    name,
    plot,
    air_date,
    video_id
)
VALUES (
    11016, 11015, 0, 'The Episode', 'An episode with no artwork of its own.',
    '2020-01-01', 11213
);

-- The resources the create use cases write: catalogue rows under the catalogue
-- container, the private rows under the library, and each typed asset owned by
-- the identity that uploaded it.
INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
VALUES
('MUSIC_GROUP', 11001, NULL, 3, 'PUBLIC'),
('MUSIC_GROUP', 11002, NULL, 3, 'PUBLIC'),
('MUSIC_ALBUM', 11003, NULL, 3, 'PUBLIC'),
('MUSIC_ALBUM', 11004, NULL, 3, 'PUBLIC'),
('MUSIC_PLAYLIST', 11008, 10000, 2, 'PRIVATE'),
('MUSIC_PLAYLIST', 11010, 10000, 2, 'PRIVATE'),
('MUSIC_PLAYLIST', 11011, 10000, 2, 'PRIVATE'),
('SERIES', 11014, NULL, 3, 'PUBLIC'),
('GALLERY', 11012, 10000, 2, 'PRIVATE'),
('IMAGE', 11201, 10000, NULL, 'PRIVATE'),
('IMAGE', 11202, 10000, NULL, 'PRIVATE'),
('IMAGE', 11203, 10000, NULL, 'PRIVATE'),
('IMAGE', 11204, 10000, NULL, 'PRIVATE'),
('IMAGE', 11205, 10000, NULL, 'PRIVATE'),
('IMAGE', 11206, 10000, NULL, 'PRIVATE'),
('IMAGE', 11207, 10000, NULL, 'PRIVATE'),
('IMAGE', 11208, 10000, NULL, 'PRIVATE'),
('IMAGE', 11209, 10000, NULL, 'PRIVATE'),
('IMAGE', 11210, 10000, NULL, 'PRIVATE'),
('AUDIO', 11211, 10000, NULL, 'PRIVATE'),
('AUDIO', 11212, 10000, NULL, 'PRIVATE'),
('VIDEO', 11213, 10000, NULL, 'PRIVATE');

-- A gallery item hangs from its gallery, whose resource identifier is assigned
-- by the table, so the parent is read back rather than written out.
INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'GALLERY_ITEM' AS kind,
    11013 AS entity_id,
    NULL AS owner_id,
    resource_id AS auth_parent_id,
    'PRIVATE' AS visibility
FROM resource
WHERE kind = 'GALLERY' AND entity_id = 11012;
