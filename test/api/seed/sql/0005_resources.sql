-- Authorization rows for the seeded catalogue, galleries and typed assets.
--
-- The create use cases write these rows as part of the operation, so the demo
-- fixture declares them too: a seeded row a route addresses must resolve to its
-- resource, or the route answers `404`. The catalogue rows hang from the
-- catalogue container and are public; a user's gallery and playlists hang from
-- the library; a typed asset records the identity that uploaded it.

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
VALUES
('MUSIC_GROUP', 9001, NULL, 3, 'PUBLIC'),
('MUSIC_ALBUM', 9001, NULL, 3, 'PUBLIC'),
('MUSIC_PLAYLIST', 9001, 9001, 2, 'PUBLIC'),
('MUSIC_PLAYLIST', 9002, 9001, 2, 'PUBLIC'),
('MUSIC_PLAYLIST', 9003, 9001, 2, 'PUBLIC'),
('MUSIC_PLAYLIST', 9004, 9001, 2, 'PUBLIC'),
('MOVIE', 9001, NULL, 3, 'PUBLIC'),
('SERIES', 9001, NULL, 3, 'PUBLIC'),
('GALLERY', 9001, 9001, 2, 'PUBLIC'),
('GALLERY', 9002, 9001, 2, 'PUBLIC'),
('IMAGE', 9001, 0, NULL, 'PRIVATE'),
('IMAGE', 9002, 0, NULL, 'PRIVATE'),
('IMAGE', 9003, 0, NULL, 'PRIVATE'),
('VIDEO', 9001, 0, NULL, 'PRIVATE'),
('VIDEO', 9002, 0, NULL, 'PRIVATE'),
('VIDEO', 9003, 0, NULL, 'PRIVATE'),
('AUDIO', 9001, 9001, NULL, 'PRIVATE')
ON CONFLICT DO NOTHING;

-- A gallery item hangs from its gallery, whose resource identifier is assigned
-- by the table, so the parent is read back rather than written out.
INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'GALLERY_ITEM' AS kind,
    9001 AS entity_id,
    NULL AS owner_id,
    resource_id AS auth_parent_id,
    'PRIVATE' AS visibility
FROM resource
WHERE kind = 'GALLERY' AND entity_id = 9001
ON CONFLICT DO NOTHING;

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'GALLERY_ITEM' AS kind,
    9002 AS entity_id,
    NULL AS owner_id,
    resource_id AS auth_parent_id,
    'PRIVATE' AS visibility
FROM resource
WHERE kind = 'GALLERY' AND entity_id = 9001
ON CONFLICT DO NOTHING;

-- The share-revocation fixture reads a stored grant by identifier. The share
-- operation upserts the same (principal, resource, role) tuple, so its
-- identifier survives a run that addresses this grant before the posting.
INSERT INTO grant (
    grant_id,
    principal_kind,
    principal_id,
    principal_role,
    resource_id,
    role,
    child_kind,
    effect
)
SELECT
    9001 AS grant_id,
    'USER' AS principal_kind,
    9002 AS principal_id,
    NULL AS principal_role,
    resource_id,
    'VIEWER' AS role,
    'ALL' AS child_kind,
    'ALLOW' AS effect
FROM resource
WHERE kind = 'GALLERY' AND entity_id = 9001
ON CONFLICT DO NOTHING;
