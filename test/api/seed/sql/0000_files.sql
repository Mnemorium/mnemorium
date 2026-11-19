-- The API fixture's file rows (identifier block 9001-9007).
--
-- The bytes are the committed media under `test/api/seed/media/`, one file per
-- row: the seed's grounding step copies each one under the library root at the
-- path the row declares and records the size and modification instant it
-- observes, so the startup scan reports every file `Untouched`. `size_bytes`
-- and `modified_at` are placeholders here because SQL cannot know them;
-- `uploaded_at` takes the column default of today, which is what a scan
-- records.
--
-- This script runs before the typed rows that reference these identifiers, so
-- the foreign keys never wait on another script.

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
    9001, 'seed/cover.png', 0, 0,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    9002, 'seed/gallery.png', 0, 0,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    9003, 'seed/playlist.png', 0, 0,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    9004, 'seed/clip.mp4', 0, 0,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    9005, 'seed/episode.mp4', 0, 0,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    9006, 'seed/gallery.mp4', 0, 0,
    'video/mp4', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    9007, 'seed/track.flac', 9001, 0,
    'audio/flac', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
);

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
VALUES
('FILE', 9001, 0, NULL, 'PRIVATE'),
('FILE', 9002, 0, NULL, 'PRIVATE'),
('FILE', 9003, 0, NULL, 'PRIVATE'),
('FILE', 9004, 0, NULL, 'PRIVATE'),
('FILE', 9005, 0, NULL, 'PRIVATE'),
('FILE', 9006, 0, NULL, 'PRIVATE'),
('FILE', 9007, 9001, NULL, 'PRIVATE')
ON CONFLICT DO NOTHING;
