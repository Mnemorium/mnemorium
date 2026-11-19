-- The system profile's movies fixture (identifier block 13000-13999).
--
-- Slots, in the layout every module block shares:
--   13000-13099  catalogue rows (films, the show and its episode)
--   13100-13199  the module's file rows
--   13200-13299  the module's typed-media rows (videos, images)
--
-- One slot per case. Each slot carries the film the case reads or patches, so a
-- case names its identifiers and never builds its preconditions over HTTP. The
-- free videos the creation cases submit arrive with the slot that proves them,
-- and the episode's video proves the attachment refusal. Every library row is
-- PUBLIC with the resource the create use case writes, so a case reads what
-- another module's fixture created.
--
-- Films and their videos:
--   13011 / 13201                            listing, covered
--   13012 / 13202                            listing, bare
--   13013 / 13203, 13014 / 13204             filtered page
--          13015 / 13205
--   13016 / 13206                            detail with full metadata
--   13017 / 13207                            no date or country
--   13018 / 13208                            repeated creation
--   13019 / 13209                            partial update
--   13020 / 13210                            cleared optional value
--
-- The show 13001 / season 13002 / episode 13003 takes video 13211, so the
-- episode attachment refusal names an already attached video. Videos
-- 13212-13217 are free, one per creation case that submits a usable video:
-- creation, unknown genre and the four unusable values.
--
-- Files (13101-13121): seventeen videos and four cover PNGs. Typed rows
-- (13201-13217 the videos, 13218-13221 the images, one per case).
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
    13101, 'system/movie-01.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13102, 'system/movie-02.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13103, 'system/movie-03.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13104, 'system/movie-04.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13105, 'system/movie-05.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13106, 'system/movie-06.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13107, 'system/movie-07.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13108, 'system/movie-08.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13109, 'system/movie-09.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13110, 'system/movie-10.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13111, 'system/movie-11.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13112, 'system/movie-12.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13113, 'system/movie-13.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13114, 'system/movie-14.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13115, 'system/movie-15.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13116, 'system/movie-16.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13117, 'system/movie-17.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13118, 'system/movie-cover-01.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13119, 'system/movie-cover-02.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13120, 'system/movie-cover-03.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    13121, 'system/movie-cover-04.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
);

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
VALUES
('FILE', 13101, 10000, NULL, 'PUBLIC'),
('FILE', 13102, 10000, NULL, 'PUBLIC'),
('FILE', 13103, 10000, NULL, 'PUBLIC'),
('FILE', 13104, 10000, NULL, 'PUBLIC'),
('FILE', 13105, 10000, NULL, 'PUBLIC'),
('FILE', 13106, 10000, NULL, 'PUBLIC'),
('FILE', 13107, 10000, NULL, 'PUBLIC'),
('FILE', 13108, 10000, NULL, 'PUBLIC'),
('FILE', 13109, 10000, NULL, 'PUBLIC'),
('FILE', 13110, 10000, NULL, 'PUBLIC'),
('FILE', 13111, 10000, NULL, 'PUBLIC'),
('FILE', 13112, 10000, NULL, 'PUBLIC'),
('FILE', 13113, 10000, NULL, 'PUBLIC'),
('FILE', 13114, 10000, NULL, 'PUBLIC'),
('FILE', 13115, 10000, NULL, 'PUBLIC'),
('FILE', 13116, 10000, NULL, 'PUBLIC'),
('FILE', 13117, 10000, NULL, 'PUBLIC'),
('FILE', 13118, 10000, NULL, 'PUBLIC'),
('FILE', 13119, 10000, NULL, 'PUBLIC'),
('FILE', 13120, 10000, NULL, 'PUBLIC'),
('FILE', 13121, 10000, NULL, 'PUBLIC');

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
VALUES
(13201, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 13101),
(13202, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 13102),
(13203, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 13103),
(13204, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 13104),
(13205, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 13105),
(13206, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 13106),
(13207, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 13107),
(13208, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 13108),
(13209, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 13109),
(13210, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 13110),
(13211, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 13111),
(13212, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 13112),
(13213, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 13113),
(13214, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 13114),
(13215, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 13115),
(13216, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 13116),
(13217, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 13117);

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
(13218, 'movie-cover-01.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 13118),
(13219, 'movie-cover-02.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 13119),
(13220, 'movie-cover-03.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 13120),
(13221, 'movie-cover-04.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 13121);

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
VALUES
('IMAGE', 13218, 10000, NULL, 'PRIVATE'),
('IMAGE', 13219, 10000, NULL, 'PRIVATE'),
('IMAGE', 13220, 10000, NULL, 'PRIVATE'),
('IMAGE', 13221, 10000, NULL, 'PRIVATE');

INSERT INTO genre (genre_id) VALUES ('COMEDY') ON CONFLICT DO NOTHING;
INSERT INTO genre (genre_id) VALUES ('DRAMA') ON CONFLICT DO NOTHING;

INSERT INTO series (
    series_id,
    name,
    plot,
    country_of_origin,
    first_air_date,
    cover_image_id
)
VALUES (13001, 'Seeded Show', '', NULL, NULL, NULL);

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
VALUES ('SERIES', 13001, NULL, 3, 'PUBLIC');

INSERT INTO season (
    season_id,
    series_id,
    season_index,
    name,
    cover_image_id
)
VALUES (13002, 13001, 0, 'Season One', NULL);

INSERT INTO episode (
    episode_id,
    season_id,
    episode_index,
    name,
    plot,
    air_date,
    video_id
)
VALUES (13003, 13002, 0, 'The Episode', '', NULL, 13211);

INSERT INTO movie (
    movie_id,
    name,
    plot,
    country_of_origin,
    release_date,
    video_id,
    cover_image_id
)
VALUES
(13011, 'Listed One', '', 'US', '1959-08-17', 13201, 13218),
(13012, 'Listed Two', '', NULL, NULL, 13202, NULL),
(13013, 'Page Film', '', NULL, NULL, 13203, NULL),
(13014, 'Page Film', '', NULL, NULL, 13204, NULL),
(13015, 'Page Film', '', NULL, NULL, 13205, NULL),
(
    13016,
    'Some Like It Hot',
    'Two musicians on the run',
    'US',
    '1959-08-17',
    13206,
    13219
),
(13017, 'Undated', '', NULL, NULL, 13207, NULL),
(13018, 'The Original', '', NULL, NULL, 13208, NULL),
(13019, 'Old Name', 'Old plot', 'US', '1959-08-17', 13209, NULL),
(13020, 'Clearable', '', 'US', '1959-08-17', 13210, 13221);

INSERT INTO movie_genre (movie_id, genre_id)
VALUES
(13016, 'COMEDY'),
(13016, 'DRAMA'),
(13019, 'DRAMA'),
(13020, 'COMEDY');

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'MOVIE' AS kind,
    movie_id AS entity_id,
    NULL AS owner_id,
    3 AS auth_parent_id,
    'PUBLIC' AS visibility
FROM movie
WHERE movie_id BETWEEN 13011 AND 13020;
