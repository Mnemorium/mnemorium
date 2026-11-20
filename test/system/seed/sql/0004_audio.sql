-- The system profile's audio fixture (identifier block 14000-14999).
--
-- Slots, in the layout every module block shares:
--   14000-14099  catalogue rows (none: audio is authorised through its file)
--   14100-14199  the module's file rows
--   14200-14299  the module's typed-media rows (the probed measurements)
--
-- The rows arrive with the six files the module commits, one per case, and
-- mirror what the probe reports for those bytes: bit depth on the 16-bit rows,
-- none on the 64-bit row, no duration on the empty row, and `block_size` NULL
-- everywhere. Audio is not a resource of its own: the read is decided by the
-- grant on the file it hangs from.
--
-- Files (14101-14106) and their audios (14201-14206):
--   audio-full.wav    one second of 16-bit PCM
--   audio-create.wav  short 16-bit, uploaded by the creation case
--   audio-dedupe.wav  short 16-bit, uploaded twice by the dedupe case
--   audio-read.wav    short 16-bit, the representation's read target
--   audio-wide.wav    short 64-bit PCM, no bit depth
--   audio-empty.wav   zero samples, no duration
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
    14101, 'system/audio-full.wav', 10000, 1,
    'audio/vnd.wave', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    14102, 'system/audio-create.wav', 10000, 1,
    'audio/vnd.wave', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    14103, 'system/audio-dedupe.wav', 10000, 1,
    'audio/vnd.wave', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    14104, 'system/audio-read.wav', 10000, 1,
    'audio/vnd.wave', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    14105, 'system/audio-wide.wav', 10000, 1,
    'audio/vnd.wave', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    14106, 'system/audio-empty.wav', 10000, 1,
    'audio/vnd.wave', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
);

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
VALUES
('FILE', 14101, 10000, NULL, 'PUBLIC'),
('FILE', 14102, 10000, NULL, 'PUBLIC'),
('FILE', 14103, 10000, NULL, 'PUBLIC'),
('FILE', 14104, 10000, NULL, 'PUBLIC'),
('FILE', 14105, 10000, NULL, 'PUBLIC'),
('FILE', 14106, 10000, NULL, 'PUBLIC');

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
(14201, 'audio-full.wav', 1000.0, 'pcm_s16le', 44100.0, 16, NULL, 14101),
(14202, 'audio-create.wav', 100.0, 'pcm_s16le', 44100.0, 16, NULL, 14102),
(14203, 'audio-dedupe.wav', 100.0, 'pcm_s16le', 44100.0, 16, NULL, 14103),
(14204, 'audio-read.wav', 100.0, 'pcm_s16le', 44100.0, 16, NULL, 14104),
(14205, 'audio-wide.wav', 100.0, 'pcm_s64le', 44100.0, NULL, NULL, 14105),
(14206, 'audio-empty.wav', NULL, 'pcm_s16le', 44100.0, 16, NULL, 14106);
