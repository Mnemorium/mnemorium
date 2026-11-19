-- The system profile's download fixture (identifier block 16000-16999).
--
-- Slots, in the layout every module block shares:
--   16000-16099  catalogue rows (none: content is addressed through its file)
--   16100-16199  the module's file rows
--   16200-16299  the module's typed-media rows
--
-- The rows arrive with the cases that read content: one read-only typed image,
-- audio and video, a typed image named `holiday.png`, a typed image named
-- `odd "name".png`, and a bare `download-bare.png` whose bytes are not a
-- decodable PNG. Every one is a PUBLIC library file. The four mutating cases
-- take one seeded file each: the image, audio and video a viewer grant reads,
-- and the image whose file download a viewer grant refuses.
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
    16101, 'system/download-image.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    16102, 'system/download-audio.flac', 10000, 1,
    'audio/flac', 0, '2020-01-01 00:00:00', 'SCANNED',
    'f6816152d401b49802bd0fa18036d7442e94e4487711f1628641805dd677be21'
),
(
    16103, 'system/download-video.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    16104, 'system/holiday.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    16105, 'system/odd "name".png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    16106, 'system/download-bare.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    16107, 'system/view-image.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    16108, 'system/download-stream-audio.flac', 10000, 1,
    'audio/flac', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    16109, 'system/download-stream-video.mp4', 10000, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    16110, 'system/refusal.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
);

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
VALUES
('FILE', 16101, 10000, NULL, 'PUBLIC'),
('FILE', 16102, 10000, NULL, 'PUBLIC'),
('FILE', 16103, 10000, NULL, 'PUBLIC'),
('FILE', 16104, 10000, NULL, 'PUBLIC'),
('FILE', 16105, 10000, NULL, 'PUBLIC'),
('FILE', 16106, 10000, NULL, 'PUBLIC'),
('FILE', 16107, 10000, NULL, 'PUBLIC'),
('FILE', 16108, 10000, NULL, 'PUBLIC'),
('FILE', 16109, 10000, NULL, 'PUBLIC'),
('FILE', 16110, 10000, NULL, 'PUBLIC');

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
(16201, 'download-image.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 16101),
(16204, 'holiday.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 16104),
(16205, 'odd "name".png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 16105),
(16206, 'view-image.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 16107),
(16209, 'refusal.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 16110);

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
(16202, 'download-audio.flac', 500.0, 'FLAC', 44100.0, 16, NULL, 16102),
(16207, 'download-stream-audio.flac', 500.0, 'FLAC', 44100.0, 16, NULL, 16108);

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
(16203, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 16103),
(16208, 200.0, 'H264', 2, 16, 16, 'YCbCr', 'PROGRESSIVE', 'LIBRARY', 16109);

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
VALUES
('IMAGE', 16201, 10000, NULL, 'PRIVATE'),
('AUDIO', 16202, 10000, NULL, 'PRIVATE'),
('VIDEO', 16203, 10000, NULL, 'PRIVATE'),
('IMAGE', 16204, 10000, NULL, 'PRIVATE'),
('IMAGE', 16205, 10000, NULL, 'PRIVATE'),
('IMAGE', 16206, 10000, NULL, 'PRIVATE'),
('AUDIO', 16207, 10000, NULL, 'PRIVATE'),
('VIDEO', 16208, 10000, NULL, 'PRIVATE'),
('IMAGE', 16209, 10000, NULL, 'PRIVATE');
