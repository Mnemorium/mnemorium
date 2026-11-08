-- Seed media rows shared by the music and video scripts.
--
-- The `file` rows these rows reference are created by the Rust seed step, which
-- owns the bytes and their metadata:
--   9001 seed/cover.png     9002 seed/gallery.png   9003 seed/playlist.png
--   9004 seed/clip.mp4      9005 seed/episode.mp4   9006 seed/gallery.mp4
--   9007 seed/track.flac
--
-- Images start with a NULL scope: the attachment triggers set LIBRARY for a
-- cover and USER for gallery or playlist media.

INSERT INTO genre (genre_id) VALUES ('ROCK'), ('JAZZ'), ('DRAMA');

INSERT INTO person (person_id, given_name, family_name, birth_date)
VALUES
(9001, 'Freddie', 'Mercury', '1946-09-05'),
(9002, 'Brian', 'May', '1947-07-19');

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
VALUES (9001, 'Seed Track', 500.0, 'FLAC', 44100.0, 16, NULL, 9007);

INSERT INTO image (
    image_id, name, width_px, height_px, orientation, created_at, file_id
)
VALUES
(9001, 'cover.png', 64, 64, 'SQUARE', '2020-01-01', 9001),
(9002, 'gallery.png', 32, 32, 'SQUARE', '2020-01-01', 9002),
(9003, 'playlist.png', 32, 32, 'SQUARE', '2020-01-01', 9003);

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
(9001, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 9004),
(9002, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 9005),
(9003, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', NULL, 9006);
