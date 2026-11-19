-- The system profile's music fixture (identifier block 12000-12999).
--
-- Slots, in the layout every module block shares:
--   12000-12499  catalogue rows (groups, albums, media, recordings, tracks)
--   12500-12599  the module's file rows
--   12600-12699  the module's typed-media rows (audio, images)
--
-- One slot per case. Each slot carries the group, album, medium or recording
-- the case acts on, so a case names its identifiers and never builds its
-- preconditions over HTTP. The recordings a case places, renames or appends
-- arrive with the slot that proves them; the default album and the default
-- group keep their fixed identifiers 0 so a case names them as the terms the
-- glossary defines. Every library row is PUBLIC with the resource the create
-- use case writes, and a recording carries the MUSIC_RECORDING resource a
-- playlist append decides `ViewMetadata` on.
--
-- Catalogue slots (group / album / medium / recording / track):
--   12001 / 12101, 12102                     album collection, covered + bare
--   12002 / 12103                            name filter
--   12003 / 12104 / 12201                    album detail metadata
--   12004 / 12105                            unreported values
--   12005                                    new album
--   12006                                    unnamed genre
--   12007 / 12106                            existing album
--   12008                                    album creation refusals
--   12009 / 12107                            release date and genres
--   12010 / 12108                            cleared optional value
--   12011 / 12109, 12012 / 12110             colliding group change
--   12013 / 12111                            absent field keeps its value
--   12014 / 12112                            medium creation
--   12015 / 12113 / 12202 / 12301 / 12401    medium retype
--   12016 / 12114 / 12203                    medium idempotence
--   12017 / 12115 / 12204                    medium read
--   12018 / 12116 / 12205                    medium refusals
--   12019 / 12117                            invalid medium type
--   12020 / 12118 / 12206 / 12302            recording placement
--   12021 / 12119 / 12207, 12208 / 12303     recording move
--   12022 / 12120 / 12209                    unknown recording
--   12023 / 12121 / 12210 / 12304            unknown destination
--   12024 / 12122 / 12211 / 12305, 12306     occupied destination
--   12025 / 12123 / 12212 / 12307 / 12404    rename reaches a playlist
--   12026 / 12124 / 12213 / 12308 / 12405    field cleared
--   12027 / 12125 / 12214 / 12309 / 12406    duplicate ISRC
--          12126 / 12215 / 12310 / 12407
--   12028 / 12127 / 12216 / 12311 / 12408    malformed ISRC
--   12029 / 12128 / 12217                    unknown track
--   12030 / 12129 / 12218 / 12312 / 12409    track read
--   12031 / 12130 / 12219 / 12313, 12314     medium track list
--          12410, 12411
--   12032 / 12131 / 12220 / 12315 / 12412    track position
--   12033                                    duplicate group
--   12034, 12035                             listed groups
--   12036                                    group detail
--   12037 / 12132                            group rename
--   12038                                    group artwork replacement
--   12039                                    group artwork clear
--   12040, 12041                             group rename collision
--   12042                                    group rename refusal
--   12043 / 12132                            album names its group
--   12044 / 12133, 12045                     album moves group
--   12134 / 12221 / 12323 / 12413            removed playlist track survivor
--   12135 / 12222 / 12331 / 12414            deleted playlist survivor
--
-- Playlist slots (recordings appended by the case's own playlist):
--   12316, 12317                             playlist detail read
--   12318, 12319                             recording appended
--   12320                                    unknown or duplicate target
--   12321                                    playlist track representation
--   12322                                    removal
--   12324-12326                              reorder
--   12327-12329                              refused permutation
--   12330                                    deletion
--
-- Files (12501-12507): `recording.flac` and six cover PNGs. Typed rows
-- (12601-12607): the six images and the one audio every recording shares.
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
    12501, 'system/recording.flac', 10000, 1,
    'audio/flac', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    12502, 'system/music-cover-01.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    12503, 'system/music-cover-02.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    12504, 'system/music-cover-03.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    12505, 'system/music-cover-04.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    12506, 'system/music-cover-05.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
),
(
    12507, 'system/music-cover-06.png', 10000, 1,
    'image/png', 0, '2020-01-01 00:00:00', 'SCANNED', NULL
);

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
VALUES
('FILE', 12501, 10000, NULL, 'PUBLIC'),
('FILE', 12502, 10000, NULL, 'PUBLIC'),
('FILE', 12503, 10000, NULL, 'PUBLIC'),
('FILE', 12504, 10000, NULL, 'PUBLIC'),
('FILE', 12505, 10000, NULL, 'PUBLIC'),
('FILE', 12506, 10000, NULL, 'PUBLIC'),
('FILE', 12507, 10000, NULL, 'PUBLIC')
ON CONFLICT DO NOTHING;

INSERT INTO genre (genre_id) VALUES ('JAZZ') ON CONFLICT DO NOTHING;

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
(12601, 'music-cover-01.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 12502),
(12602, 'music-cover-02.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 12503),
(12603, 'music-cover-03.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 12504),
(12604, 'music-cover-04.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 12505),
(12605, 'music-cover-05.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 12506),
(12606, 'music-cover-06.png', 64, 64, 'SQUARE', '2020-01-01', 'LIBRARY', 12507);

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
VALUES (12607, 'recording.flac', 500.0, 'FLAC', 44100.0, 16, NULL, 12501);

INSERT INTO music_group (music_group_id, stage_name, cover_image_id)
VALUES
(12001, 'Collection Band', NULL),
(12002, 'Filter Band', NULL),
(12003, 'Detail Band', NULL),
(12004, 'Bare Band', NULL),
(12005, 'New Album Band', NULL),
(12006, 'Unnamed Genre Band', NULL),
(12007, 'Existing Album Band', NULL),
(12008, 'Refusal Band', NULL),
(12009, 'Updated Band', NULL),
(12010, 'Clear Band', NULL),
(12011, 'Collide One', NULL),
(12012, 'Collide Two', NULL),
(12013, 'Absent Band', NULL),
(12014, 'Medium Band', NULL),
(12015, 'Retype Band', NULL),
(12016, 'Idempotent Band', NULL),
(12017, 'Medium Read Band', NULL),
(12018, 'Medium Refusal Band', NULL),
(12019, 'Invalid Medium Band', NULL),
(12020, 'Placed Band', NULL),
(12021, 'Moved Band', NULL),
(12022, 'Unknown Recording Band', NULL),
(12023, 'Unknown Destination Band', NULL),
(12024, 'Occupied Band', NULL),
(12025, 'Rename Band', NULL),
(12026, 'Clear Field Band', NULL),
(12027, 'Isrc Band', NULL),
(12028, 'Malformed Isrc Band', NULL),
(12029, 'Unknown Track Band', NULL),
(12030, 'Track Read Band', NULL),
(12031, 'Track List Band', NULL),
(12032, 'Position Band', NULL),
(12033, 'Duplicate Group', NULL),
(12034, 'Listed One', 12605),
(12035, 'Listed Two', NULL),
(12036, 'Detail Group', 12606),
(12037, 'Rename Keep Group', 12601),
(12038, 'Replace Artwork Group', 12602),
(12039, 'Clear Artwork Group', 12604),
(12040, 'Taken Group', NULL),
(12041, 'Renamed Group', NULL),
(12042, 'Invalid Name Group', NULL),
(12043, 'Album Group', NULL),
(12044, 'Move Source Group', NULL),
(12045, 'Move Target Group', NULL),
(12046, 'Survivor Band', NULL),
(12047, 'Deleted Survivor Band', NULL),
(12048, 'Collection Two Band', NULL);

INSERT INTO music_album (
    music_album_id,
    name,
    release_date,
    production_type,
    cover_image_id,
    music_group_id
)
VALUES
(12101, 'Collected Album', NULL, NULL, 12601, 12001),
(12102, 'Collected Album', NULL, NULL, NULL, 12048),
(12103, 'Filtered Album', NULL, NULL, NULL, 12002),
(12104, 'Detailed Album', '1970-01-01', 'LIVE', 12602, 12003),
(12105, 'Bare Album', NULL, NULL, NULL, 12004),
(12106, 'Existing Album', NULL, NULL, NULL, 12007),
(12107, 'Updated Album', NULL, NULL, NULL, 12009),
(12108, 'Clear Album', '1970-01-01', NULL, NULL, 12010),
(12109, 'Shared Collision', NULL, NULL, NULL, 12011),
(12110, 'Shared Collision', NULL, NULL, NULL, 12012),
(12111, 'Kept Album', NULL, 'LIVE', NULL, 12013),
(12112, 'Medium Album', NULL, NULL, NULL, 12014),
(12113, 'Retype Album', NULL, NULL, NULL, 12015),
(12114, 'Idempotent Album', NULL, NULL, NULL, 12016),
(12115, 'Medium Read Album', NULL, NULL, NULL, 12017),
(12116, 'Medium Refusal Album', NULL, NULL, NULL, 12018),
(12117, 'Invalid Medium Album', NULL, NULL, NULL, 12019),
(12118, 'Placed Album', NULL, NULL, NULL, 12020),
(12119, 'Moved Album', NULL, NULL, NULL, 12021),
(12120, 'Unknown Recording Album', NULL, NULL, NULL, 12022),
(12121, 'Unknown Destination Album', NULL, NULL, NULL, 12023),
(12122, 'Occupied Album', NULL, NULL, NULL, 12024),
(12123, 'Rename Album', NULL, NULL, NULL, 12025),
(12124, 'Clear Field Album', NULL, NULL, NULL, 12026),
(12125, 'Isrc Album', NULL, NULL, NULL, 12027),
(12126, 'Second Isrc Album', NULL, NULL, NULL, 12027),
(12127, 'Malformed Isrc Album', NULL, NULL, NULL, 12028),
(12128, 'Unknown Track Album', NULL, NULL, NULL, 12029),
(12129, 'Track Read Album', NULL, NULL, 12603, 12030),
(12130, 'Track List Album', NULL, NULL, NULL, 12031),
(12131, 'Position Album', NULL, NULL, NULL, 12032),
(12132, 'Album Naming Group', NULL, NULL, NULL, 12043),
(12133, 'Moving Album', NULL, NULL, NULL, 12044),
(12134, 'Survivor Album', NULL, NULL, NULL, 12046),
(12135, 'Deleted Survivor Album', NULL, NULL, NULL, 12047);

INSERT INTO music_album_genre (music_album_id, genre_id)
SELECT
    music_album_id,
    'UNKNOWN' AS genre_id
FROM music_album
WHERE music_album_id BETWEEN 12101 AND 12135 AND music_album_id <> 12104;

INSERT INTO music_album_genre (music_album_id, genre_id)
VALUES (12104, 'JAZZ');

INSERT INTO music_medium (music_medium_id, music_album_id, type, medium_index)
VALUES
(12201, 12104, 'CD', 1),
(12202, 12113, 'CD', 1),
(12203, 12114, 'DIGITAL', 1),
(12204, 12115, 'CD', 1),
(12205, 12116, 'DIGITAL', 1),
(12206, 12118, 'DIGITAL', 1),
(12207, 12119, 'DIGITAL', 1),
(12208, 12119, 'CD', 2),
(12209, 12120, 'DIGITAL', 1),
(12210, 12121, 'DIGITAL', 1),
(12211, 12122, 'DIGITAL', 1),
(12212, 12123, 'DIGITAL', 1),
(12213, 12124, 'DIGITAL', 1),
(12214, 12125, 'DIGITAL', 1),
(12215, 12126, 'DIGITAL', 1),
(12216, 12127, 'DIGITAL', 1),
(12217, 12128, 'DIGITAL', 1),
(12218, 12129, 'DIGITAL', 1),
(12219, 12130, 'DIGITAL', 1),
(12220, 12131, 'DIGITAL', 1),
(12221, 12134, 'DIGITAL', 1),
(12222, 12135, 'DIGITAL', 1);

INSERT INTO music_recording (music_recording_id, name, isrc_code, audio_id)
VALUES
(12301, 'Retype Song', NULL, 12607),
(12302, 'Place Me', NULL, 12607),
(12303, 'Move Me', NULL, 12607),
(12304, 'Destination Target', NULL, 12607),
(12305, 'Occupied Seat', NULL, 12607),
(12306, 'Occupant', NULL, 12607),
(12307, 'Before Rename', NULL, 12607),
(12308, 'Clearable', 'USRC17607840', 12607),
(12309, 'Isrc One', NULL, 12607),
(12310, 'Isrc Two', NULL, 12607),
(12311, 'Malformed Isrc', NULL, 12607),
(12312, 'Readable Track', 'USRC17607842', 12607),
(12313, 'Listed One', NULL, 12607),
(12314, 'Listed Two', NULL, 12607),
(12315, 'Positioned', NULL, 12607),
(12316, 'Playlist One', 'USRC17607843', 12607),
(12317, 'Playlist Two', NULL, 12607),
(12318, 'Append One', NULL, 12607),
(12319, 'Append Two', NULL, 12607),
(12320, 'Target Known', NULL, 12607),
(12321, 'Track With Name', 'USRC17607844', 12607),
(12322, 'Remove Me', NULL, 12607),
(12323, 'Survives Removal', NULL, 12607),
(12324, 'Order A', NULL, 12607),
(12325, 'Order B', NULL, 12607),
(12326, 'Order C', NULL, 12607),
(12327, 'Permutation A', NULL, 12607),
(12328, 'Permutation B', NULL, 12607),
(12329, 'Permutation C', NULL, 12607),
(12330, 'Delete Me', NULL, 12607),
(12331, 'Survives Delete', NULL, 12607);

INSERT INTO music_track (
    music_track_id,
    track_index,
    music_playlist_id,
    music_medium_id,
    music_recording_id
)
VALUES
(12401, 0, NULL, 12202, 12301),
(12402, 0, NULL, 12207, 12303),
(12403, 0, NULL, 12211, 12305),
(12404, 0, NULL, 12212, 12307),
(12405, 0, NULL, 12213, 12308),
(12406, 0, NULL, 12214, 12309),
(12407, 0, NULL, 12215, 12310),
(12408, 0, NULL, 12216, 12311),
(12409, 0, NULL, 12218, 12312),
(12410, 0, NULL, 12219, 12313),
(12411, 1, NULL, 12219, 12314),
(12412, 0, NULL, 12220, 12315),
(12413, 0, NULL, 12221, 12323),
(12414, 0, NULL, 12222, 12331);

-- The resources the create use cases write: catalogue rows under the catalogue
-- container, the recordings a playlist append decides on, and each typed asset
-- owned by the identity that uploaded it.
INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'MUSIC_GROUP' AS kind,
    music_group_id AS entity_id,
    NULL AS owner_id,
    3 AS auth_parent_id,
    'PUBLIC' AS visibility
FROM music_group
WHERE music_group_id BETWEEN 12001 AND 12048;

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'MUSIC_ALBUM' AS kind,
    music_album_id AS entity_id,
    NULL AS owner_id,
    3 AS auth_parent_id,
    'PUBLIC' AS visibility
FROM music_album
WHERE music_album_id BETWEEN 12101 AND 12135;

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'MUSIC_RECORDING' AS kind,
    music_recording_id AS entity_id,
    NULL AS owner_id,
    3 AS auth_parent_id,
    'PUBLIC' AS visibility
FROM music_recording
WHERE music_recording_id BETWEEN 12301 AND 12331;

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'IMAGE' AS kind,
    image_id AS entity_id,
    10000 AS owner_id,
    NULL AS auth_parent_id,
    'PRIVATE' AS visibility
FROM image
WHERE image_id BETWEEN 12601 AND 12606;

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
VALUES ('AUDIO', 12607, 10000, NULL, 'PRIVATE');
