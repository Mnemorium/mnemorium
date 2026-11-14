-- The backfill selects positional values from the source tables: the INSERT
-- column list names each target, so an explicit alias per select target would
-- only repeat it. The aliasing rules are disabled for this file alone.
-- noqa:disable=AL03,AL08
CREATE TABLE resource (
    resource_id INTEGER,
    kind TEXT NOT NULL,
    entity_id INTEGER NOT NULL,
    owner_id INTEGER,
    auth_parent_id INTEGER,
    visibility TEXT NOT NULL DEFAULT 'PRIVATE',
    CONSTRAINT pk_resource_resource_id PRIMARY KEY (resource_id),
    CONSTRAINT uq_resource_kind_entity_id UNIQUE (kind, entity_id),
    CONSTRAINT fk_resource_owner FOREIGN KEY (
        owner_id
    ) REFERENCES user (user_id) ON DELETE CASCADE,
    CONSTRAINT fk_resource_auth_parent FOREIGN KEY (
        auth_parent_id
    ) REFERENCES resource (resource_id) ON DELETE CASCADE,
    CONSTRAINT chk_resource_kind CHECK (
        kind IN (
            'INSTANCE', 'LIBRARY', 'CATALOGUE', 'USER', 'UPLOAD',
            'GALLERY', 'GALLERY_ITEM', 'MOVIE', 'SERIES', 'SEASON', 'EPISODE',
            'MUSIC_GROUP', 'MUSIC_ALBUM', 'MUSIC_MEDIUM', 'MUSIC_RECORDING',
            'MUSIC_PLAYLIST', 'IMAGE', 'VIDEO', 'AUDIO', 'FILE',
            'GALLERY_COLLECTION', 'PLAYLIST_COLLECTION', 'UPLOAD_COLLECTION',
            'USER_COLLECTION'
        )
    ),
    CONSTRAINT chk_resource_visibility CHECK (
        visibility IN ('PRIVATE', 'RESTRICTED', 'PUBLIC')
    ),
    CONSTRAINT chk_resource_no_self_parent CHECK (
        auth_parent_id IS NULL OR auth_parent_id <> resource_id
    ),
    CONSTRAINT chk_resource_user_has_no_owner CHECK (
        kind <> 'USER' OR owner_id IS NULL
    ),
    CONSTRAINT chk_resource_catalogue_has_no_owner CHECK (
        kind NOT IN (
            'CATALOGUE', 'MOVIE', 'SERIES', 'SEASON', 'EPISODE',
            'MUSIC_GROUP', 'MUSIC_ALBUM', 'MUSIC_MEDIUM', 'MUSIC_RECORDING'
        ) OR owner_id IS NULL
    )
);

CREATE TABLE resource_relation (
    from_resource_id INTEGER NOT NULL,
    to_resource_id INTEGER NOT NULL,
    kind TEXT NOT NULL,
    position INTEGER,
    CONSTRAINT pk_resource_relation PRIMARY KEY (
        from_resource_id, to_resource_id, kind
    ),
    CONSTRAINT fk_resource_relation_from FOREIGN KEY (
        from_resource_id
    ) REFERENCES resource (resource_id) ON DELETE CASCADE,
    CONSTRAINT fk_resource_relation_to FOREIGN KEY (
        to_resource_id
    ) REFERENCES resource (resource_id) ON DELETE CASCADE,
    CONSTRAINT chk_resource_relation_kind CHECK (
        kind IN ('ALBUM_MEMBER', 'PLAYLIST_ENTRY', 'GROUP_MEMBER')
    ),
    CONSTRAINT chk_resource_relation_position CHECK (
        position IS NULL OR position >= 0
    ),
    CONSTRAINT chk_resource_relation_no_self CHECK (
        from_resource_id <> to_resource_id
    )
);

CREATE UNIQUE INDEX uq_resource_relation_position
ON resource_relation (from_resource_id, kind, position)
WHERE position IS NOT NULL;

CREATE INDEX idx_resource_relation_to
ON resource_relation (to_resource_id);

CREATE TABLE grant (
    grant_id INTEGER,
    principal_kind TEXT NOT NULL,
    principal_id INTEGER,
    principal_role VARCHAR(50),
    resource_id INTEGER NOT NULL,
    role TEXT NOT NULL,
    child_kind TEXT NOT NULL DEFAULT 'ALL',
    effect TEXT NOT NULL,
    CONSTRAINT pk_grant_grant_id PRIMARY KEY (grant_id),
    CONSTRAINT fk_grant_resource FOREIGN KEY (
        resource_id
    ) REFERENCES resource (resource_id) ON DELETE CASCADE,
    CONSTRAINT chk_grant_principal_kind CHECK (
        principal_kind IN ('USER', 'ROLE')
    ),
    CONSTRAINT chk_grant_principal CHECK (
        (
            principal_kind = 'USER'
            AND principal_id > 0
            AND principal_role IS NULL
        )
        OR (
            principal_kind = 'ROLE'
            AND principal_id IS NULL
            AND principal_role IN ('ADMIN', 'STANDARD')
        )
    ),
    CONSTRAINT chk_grant_role CHECK (
        role IN (
            'VIEWER', 'DOWNLOADER', 'CONTRIBUTOR', 'EDITOR', 'MANAGER', 'OWNER'
        )
    ),
    CONSTRAINT chk_grant_owner_is_a_role CHECK (
        role <> 'OWNER' OR principal_kind = 'ROLE'
    ),
    CONSTRAINT chk_grant_child_kind CHECK (
        child_kind IN (
            'ALL', 'GALLERY', 'MUSIC_PLAYLIST', 'UPLOAD', 'USER', 'ADMIN',
            'MOVIE', 'SERIES', 'SEASON', 'EPISODE', 'MUSIC_GROUP',
            'MUSIC_ALBUM', 'MUSIC_MEDIUM', 'MUSIC_RECORDING', 'GALLERY_ITEM',
            'IMAGE', 'VIDEO', 'AUDIO', 'FILE'
        )
    ),
    CONSTRAINT chk_grant_admin_is_not_grantable CHECK (
        child_kind <> 'ADMIN'
    ),
    CONSTRAINT chk_grant_effect CHECK (effect IN ('ALLOW', 'DENY'))
);

CREATE UNIQUE INDEX uq_grant_user
ON grant (principal_id, resource_id, role, child_kind, effect)
WHERE principal_kind = 'USER';

CREATE UNIQUE INDEX uq_grant_role
ON grant (principal_role, resource_id, role, child_kind, effect)
WHERE principal_kind = 'ROLE';

CREATE INDEX idx_grant_resource ON grant (resource_id);

-- Singletons and creation surfaces.
INSERT INTO resource (
    resource_id, kind, entity_id, owner_id, auth_parent_id, visibility
) VALUES
(1, 'INSTANCE', 0, NULL, NULL, 'PUBLIC'),
(2, 'LIBRARY', 0, NULL, 1, 'PUBLIC'),
(3, 'CATALOGUE', 0, NULL, 2, 'PUBLIC'),
(4, 'GALLERY_COLLECTION', 0, NULL, 2, 'PRIVATE'),
(5, 'PLAYLIST_COLLECTION', 0, NULL, 2, 'PRIVATE'),
(6, 'UPLOAD_COLLECTION', 0, NULL, 2, 'PRIVATE'),
(7, 'USER_COLLECTION', 0, NULL, 1, 'PRIVATE');

-- User content under the library.
INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'GALLERY',
    gallery.gallery_id,
    gallery.user_id,
    2,
    CASE WHEN gallery.is_public THEN 'PUBLIC' ELSE 'PRIVATE' END
FROM gallery;

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'MUSIC_PLAYLIST',
    music_playlist.music_playlist_id,
    music_playlist.user_id,
    2,
    CASE WHEN music_playlist.is_public THEN 'PUBLIC' ELSE 'PRIVATE' END
FROM music_playlist;

-- Accounts and uploads hang from the instance root.
INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'USER',
    user.user_id,
    NULL,
    1,
    'PRIVATE'
FROM user;

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'UPLOAD',
    upload.upload_id,
    upload.user_id,
    1,
    'PRIVATE'
FROM upload;

-- Shared catalogue under the catalogue container.
INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'MOVIE',
    movie.movie_id,
    NULL,
    3,
    'PUBLIC'
FROM movie;

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'SERIES',
    series.series_id,
    NULL,
    3,
    'PUBLIC'
FROM series;

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'SEASON',
    season.season_id,
    NULL,
    parent.resource_id,
    'PUBLIC'
FROM season
INNER JOIN resource AS parent
    ON parent.kind = 'SERIES' AND season.series_id = parent.entity_id;

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'EPISODE',
    episode.episode_id,
    NULL,
    parent.resource_id,
    'PUBLIC'
FROM episode
INNER JOIN resource AS parent
    ON parent.kind = 'SEASON' AND episode.season_id = parent.entity_id;

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'MUSIC_GROUP',
    music_group.music_group_id,
    NULL,
    3,
    'PUBLIC'
FROM music_group;

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'MUSIC_ALBUM',
    music_album.music_album_id,
    NULL,
    3,
    'PUBLIC'
FROM music_album;

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'MUSIC_MEDIUM',
    music_medium.music_medium_id,
    NULL,
    parent.resource_id,
    'PUBLIC'
FROM music_medium
INNER JOIN resource AS parent
    ON
        parent.kind = 'MUSIC_ALBUM'
        AND music_medium.music_album_id = parent.entity_id;

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'MUSIC_RECORDING',
    music_recording.music_recording_id,
    NULL,
    COALESCE(
        (
            SELECT medium.resource_id
            FROM music_track
            INNER JOIN resource AS medium
                ON
                    medium.kind = 'MUSIC_MEDIUM'
                    AND music_track.music_medium_id = medium.entity_id
            WHERE
                music_track.music_recording_id
                = music_recording.music_recording_id
            ORDER BY music_track.music_medium_id
            LIMIT 1
        ),
        3
    ),
    'PUBLIC'
FROM music_recording;

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'GALLERY_ITEM',
    gallery_item.gallery_item_id,
    NULL,
    parent.resource_id,
    'PRIVATE'
FROM gallery_item
INNER JOIN resource AS parent
    ON parent.kind = 'GALLERY' AND gallery_item.gallery_id = parent.entity_id;

-- Typed assets follow the resource that names them.
INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'IMAGE',
    image.image_id,
    file.user_id,
    NULL,
    'PRIVATE'
FROM image
INNER JOIN file ON image.file_id = file.file_id;

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'VIDEO',
    video.video_id,
    file.user_id,
    NULL,
    'PRIVATE'
FROM video
INNER JOIN file ON video.file_id = file.file_id;

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'AUDIO',
    audio.audio_id,
    file.user_id,
    NULL,
    'PRIVATE'
FROM audio
INNER JOIN file ON audio.file_id = file.file_id;

UPDATE resource
SET
    auth_parent_id = (
        SELECT parent.resource_id
        FROM gallery_item
        INNER JOIN resource AS parent
            ON
                parent.kind = 'GALLERY_ITEM'
                AND gallery_item.gallery_item_id = parent.entity_id
        WHERE gallery_item.image_id = resource.entity_id
    )
WHERE
    kind = 'IMAGE' AND auth_parent_id IS NULL
    AND EXISTS (
        SELECT 1
        FROM gallery_item
        WHERE gallery_item.image_id = resource.entity_id
    );

UPDATE resource
SET
    auth_parent_id = (
        SELECT parent.resource_id
        FROM music_playlist
        INNER JOIN resource AS parent
            ON
                parent.kind = 'MUSIC_PLAYLIST'
                AND music_playlist.music_playlist_id = parent.entity_id
        WHERE music_playlist.cover_image_id = resource.entity_id
    )
WHERE
    kind = 'IMAGE' AND auth_parent_id IS NULL
    AND EXISTS (
        SELECT 1
        FROM music_playlist
        WHERE music_playlist.cover_image_id = resource.entity_id
    );

UPDATE resource
SET
    auth_parent_id = (
        SELECT parent.resource_id
        FROM movie
        INNER JOIN resource AS parent
            ON parent.kind = 'MOVIE' AND movie.movie_id = parent.entity_id
        WHERE movie.cover_image_id = resource.entity_id
    )
WHERE
    kind = 'IMAGE' AND auth_parent_id IS NULL
    AND EXISTS (
        SELECT 1 FROM movie
        WHERE movie.cover_image_id = resource.entity_id
    );

UPDATE resource
SET
    auth_parent_id = (
        SELECT parent.resource_id
        FROM series
        INNER JOIN resource AS parent
            ON parent.kind = 'SERIES' AND series.series_id = parent.entity_id
        WHERE series.cover_image_id = resource.entity_id
    )
WHERE
    kind = 'IMAGE' AND auth_parent_id IS NULL
    AND EXISTS (
        SELECT 1 FROM series
        WHERE series.cover_image_id = resource.entity_id
    );

UPDATE resource
SET
    auth_parent_id = (
        SELECT parent.resource_id
        FROM season
        INNER JOIN resource AS parent
            ON parent.kind = 'SEASON' AND season.season_id = parent.entity_id
        WHERE season.cover_image_id = resource.entity_id
    )
WHERE
    kind = 'IMAGE' AND auth_parent_id IS NULL
    AND EXISTS (
        SELECT 1 FROM season
        WHERE season.cover_image_id = resource.entity_id
    );

UPDATE resource
SET
    auth_parent_id = (
        SELECT parent.resource_id
        FROM music_album
        INNER JOIN resource AS parent
            ON
                parent.kind = 'MUSIC_ALBUM'
                AND music_album.music_album_id = parent.entity_id
        WHERE music_album.cover_image_id = resource.entity_id
    )
WHERE
    kind = 'IMAGE' AND auth_parent_id IS NULL
    AND EXISTS (
        SELECT 1
        FROM music_album
        WHERE music_album.cover_image_id = resource.entity_id
    );

UPDATE resource
SET
    auth_parent_id = (
        SELECT parent.resource_id
        FROM music_group
        INNER JOIN resource AS parent
            ON
                parent.kind = 'MUSIC_GROUP'
                AND music_group.music_group_id = parent.entity_id
        WHERE music_group.cover_image_id = resource.entity_id
    )
WHERE
    kind = 'IMAGE' AND auth_parent_id IS NULL
    AND EXISTS (
        SELECT 1
        FROM music_group
        WHERE music_group.cover_image_id = resource.entity_id
    );

UPDATE resource
SET
    auth_parent_id = (
        SELECT parent.resource_id
        FROM gallery_item
        INNER JOIN resource AS parent
            ON
                parent.kind = 'GALLERY_ITEM'
                AND gallery_item.gallery_item_id = parent.entity_id
        WHERE gallery_item.video_id = resource.entity_id
    )
WHERE
    kind = 'VIDEO' AND auth_parent_id IS NULL
    AND EXISTS (
        SELECT 1
        FROM gallery_item
        WHERE gallery_item.video_id = resource.entity_id
    );

UPDATE resource
SET
    auth_parent_id = (
        SELECT parent.resource_id
        FROM movie
        INNER JOIN resource AS parent
            ON parent.kind = 'MOVIE' AND movie.movie_id = parent.entity_id
        WHERE movie.video_id = resource.entity_id
    )
WHERE
    kind = 'VIDEO' AND auth_parent_id IS NULL
    AND EXISTS (
        SELECT 1 FROM movie
        WHERE movie.video_id = resource.entity_id
    );

UPDATE resource
SET
    auth_parent_id = (
        SELECT parent.resource_id
        FROM episode
        INNER JOIN resource AS parent
            ON parent.kind = 'EPISODE' AND episode.episode_id = parent.entity_id
        WHERE episode.video_id = resource.entity_id
    )
WHERE
    kind = 'VIDEO' AND auth_parent_id IS NULL
    AND EXISTS (
        SELECT 1 FROM episode
        WHERE episode.video_id = resource.entity_id
    );

UPDATE resource
SET
    auth_parent_id = (
        SELECT parent.resource_id
        FROM music_recording
        INNER JOIN resource AS parent
            ON
                parent.kind = 'MUSIC_RECORDING'
                AND music_recording.music_recording_id = parent.entity_id
        WHERE music_recording.audio_id = resource.entity_id
    )
WHERE
    kind = 'AUDIO' AND auth_parent_id IS NULL
    AND EXISTS (
        SELECT 1
        FROM music_recording
        WHERE music_recording.audio_id = resource.entity_id
    );

INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)
SELECT
    'FILE',
    file.file_id,
    file.user_id,
    COALESCE(
        (
            SELECT parent.resource_id
            FROM image
            INNER JOIN resource AS parent
                ON parent.kind = 'IMAGE' AND image.image_id = parent.entity_id
            WHERE image.file_id = file.file_id
        ),
        (
            SELECT parent.resource_id
            FROM video
            INNER JOIN resource AS parent
                ON parent.kind = 'VIDEO' AND video.video_id = parent.entity_id
            WHERE video.file_id = file.file_id
        ),
        (
            SELECT parent.resource_id
            FROM audio
            INNER JOIN resource AS parent
                ON parent.kind = 'AUDIO' AND audio.audio_id = parent.entity_id
            WHERE audio.file_id = file.file_id
        )
    ),
    'PRIVATE'
FROM file;

-- Containment and reference edges.
INSERT OR IGNORE INTO resource_relation (
    from_resource_id,
    to_resource_id,
    kind,
    position
)
SELECT
    album.resource_id,
    recording.resource_id,
    'ALBUM_MEMBER',
    music_track.track_index
FROM music_track
INNER JOIN music_medium
    ON music_track.music_medium_id = music_medium.music_medium_id
INNER JOIN resource AS album
    ON
        album.kind = 'MUSIC_ALBUM'
        AND music_medium.music_album_id = album.entity_id
INNER JOIN resource AS recording
    ON
        recording.kind = 'MUSIC_RECORDING'
        AND music_track.music_recording_id = recording.entity_id;

INSERT OR IGNORE INTO resource_relation (
    from_resource_id,
    to_resource_id,
    kind,
    position
)
SELECT
    playlist.resource_id,
    recording.resource_id,
    'PLAYLIST_ENTRY',
    music_track.track_index
FROM music_track
INNER JOIN resource AS playlist
    ON
        playlist.kind = 'MUSIC_PLAYLIST'
        AND music_track.music_playlist_id = playlist.entity_id
INNER JOIN resource AS recording
    ON
        recording.kind = 'MUSIC_RECORDING'
        AND music_track.music_recording_id = recording.entity_id;

INSERT OR IGNORE INTO resource_relation (
    from_resource_id,
    to_resource_id,
    kind,
    position
)
SELECT
    music_group.resource_id,
    album.resource_id,
    'GROUP_MEMBER',
    NULL
FROM music_album_group
INNER JOIN resource AS music_group
    ON
        music_group.kind = 'MUSIC_GROUP'
        AND music_album_group.music_group_id = music_group.entity_id
INNER JOIN resource AS album
    ON
        album.kind = 'MUSIC_ALBUM'
        AND music_album_group.music_album_id = album.entity_id;

-- Seeded role grants.
INSERT INTO grant (
    principal_kind,
    principal_id,
    principal_role,
    resource_id,
    role,
    child_kind,
    effect
)
SELECT
    'ROLE',
    NULL,
    'STANDARD',
    resource_id,
    'VIEWER',
    'ALL',
    'ALLOW'
FROM resource
WHERE kind = 'CATALOGUE';

INSERT INTO grant (
    principal_kind,
    principal_id,
    principal_role,
    resource_id,
    role,
    child_kind,
    effect
)
SELECT
    'ROLE',
    NULL,
    'STANDARD',
    resource_id,
    'CONTRIBUTOR',
    'GALLERY',
    'ALLOW'
FROM resource
WHERE kind = 'GALLERY_COLLECTION';

INSERT INTO grant (
    principal_kind,
    principal_id,
    principal_role,
    resource_id,
    role,
    child_kind,
    effect
)
SELECT
    'ROLE',
    NULL,
    'STANDARD',
    resource_id,
    'CONTRIBUTOR',
    'MUSIC_PLAYLIST',
    'ALLOW'
FROM resource
WHERE kind = 'PLAYLIST_COLLECTION';

INSERT INTO grant (
    principal_kind,
    principal_id,
    principal_role,
    resource_id,
    role,
    child_kind,
    effect
)
SELECT
    'ROLE',
    NULL,
    'STANDARD',
    resource_id,
    'CONTRIBUTOR',
    'UPLOAD',
    'ALLOW'
FROM resource
WHERE kind = 'UPLOAD_COLLECTION';

INSERT INTO grant (
    principal_kind,
    principal_id,
    principal_role,
    resource_id,
    role,
    child_kind,
    effect
)
SELECT
    'ROLE',
    NULL,
    'ADMIN',
    resource_id,
    'OWNER',
    'ALL',
    'ALLOW'
FROM resource
WHERE kind = 'CATALOGUE';

INSERT INTO grant (
    principal_kind,
    principal_id,
    principal_role,
    resource_id,
    role,
    child_kind,
    effect
)
SELECT
    'ROLE',
    NULL,
    'ADMIN',
    resource_id,
    'CONTRIBUTOR',
    'GALLERY',
    'ALLOW'
FROM resource
WHERE kind = 'GALLERY_COLLECTION';

INSERT INTO grant (
    principal_kind,
    principal_id,
    principal_role,
    resource_id,
    role,
    child_kind,
    effect
)
SELECT
    'ROLE',
    NULL,
    'ADMIN',
    resource_id,
    'CONTRIBUTOR',
    'MUSIC_PLAYLIST',
    'ALLOW'
FROM resource
WHERE kind = 'PLAYLIST_COLLECTION';

INSERT INTO grant (
    principal_kind,
    principal_id,
    principal_role,
    resource_id,
    role,
    child_kind,
    effect
)
SELECT
    'ROLE',
    NULL,
    'ADMIN',
    resource_id,
    'CONTRIBUTOR',
    'USER',
    'ALLOW'
FROM resource
WHERE kind = 'USER_COLLECTION';

INSERT INTO grant (
    principal_kind,
    principal_id,
    principal_role,
    resource_id,
    role,
    child_kind,
    effect
)
SELECT
    'ROLE',
    NULL,
    'ADMIN',
    resource_id,
    'OWNER',
    'ALL',
    'ALLOW'
FROM resource
WHERE kind = 'USER';
