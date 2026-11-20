-- The system profile's stored-file fixture (identifier block 15000-15999).
--
-- Slots, in the layout every module block shares:
--   15000-15099  the stored `file` rows
--   15100-15199  the untyped file that answers a scan-only record
--   15200-15299  the typed-media rows (`image`, `audio`, `video`)
--
-- The four rows arrive with the cases that read the stored-file collection:
-- `file-linked.png` (image 15201, 64x64 SQUARE), `file-audio.flac` (audio
-- 15202), `file-video.mp4` (video 15203) and `file-bare.png`, which has no
-- typed row. Every row is owned by the Root Admin, PUBLIC, with the bytes
-- committed and the typed rows mirroring what the probe reports.
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
    15001, 'system/file-linked.png', 0, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    15002, 'system/file-audio.flac', 0, 1,
    'audio/flac', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    15003, 'system/file-video.mp4', 0, 1,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    15004, 'system/file-bare.png', 0, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
);

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
VALUES
('FILE', 15001, 0, NULL, 'PUBLIC'),
('FILE', 15002, 0, NULL, 'PUBLIC'),
('FILE', 15003, 0, NULL, 'PUBLIC'),
('FILE', 15004, 0, NULL, 'PUBLIC');

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
VALUES (
    15201,
    'file-linked.png',
    64,
    64,
    'SQUARE',
    '2020-01-01',
    'LIBRARY',
    15001
);

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
VALUES (
    15202,
    'file-audio.flac',
    500.0,
    'FLAC',
    44100.0,
    16,
    NULL,
    15002
);

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
    15203,
    200.0,
    'H264',
    2,
    16,
    16,
    'YCbCr',
    'PROGRESSIVE',
    'LIBRARY',
    15003
);

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
VALUES
('IMAGE', 15201, 0, NULL, 'PRIVATE'),
('AUDIO', 15202, 0, NULL, 'PRIVATE'),
('VIDEO', 15203, 0, NULL, 'PRIVATE');
