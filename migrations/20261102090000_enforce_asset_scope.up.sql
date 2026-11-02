-- A library cover may only reference a LIBRARY image. The first attachment
-- decides the scope; a later attachment in the other direction is a conflict.
CREATE TRIGGER tg_music_group_cover_scope_insert
BEFORE INSERT ON music_group
FOR EACH ROW
WHEN new.cover_image_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'a library cover requires a LIBRARY image')
    WHERE (
        SELECT image.scope
        FROM image
        WHERE image.image_id = new.cover_image_id
    )
    = 'USER';

    UPDATE image
    SET scope = 'LIBRARY'
    WHERE image.image_id = new.cover_image_id AND image.scope IS NULL;
END;

CREATE TRIGGER tg_music_group_cover_scope_update
BEFORE UPDATE OF cover_image_id ON music_group
FOR EACH ROW
WHEN new.cover_image_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'a library cover requires a LIBRARY image')
    WHERE (
        SELECT image.scope
        FROM image
        WHERE image.image_id = new.cover_image_id
    )
    = 'USER';

    UPDATE image
    SET scope = 'LIBRARY'
    WHERE image.image_id = new.cover_image_id AND image.scope IS NULL;
END;

CREATE TRIGGER tg_music_album_cover_scope_insert
BEFORE INSERT ON music_album
FOR EACH ROW
WHEN new.cover_image_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'a library cover requires a LIBRARY image')
    WHERE (
        SELECT image.scope
        FROM image
        WHERE image.image_id = new.cover_image_id
    )
    = 'USER';

    UPDATE image
    SET scope = 'LIBRARY'
    WHERE image.image_id = new.cover_image_id AND image.scope IS NULL;
END;

CREATE TRIGGER tg_music_album_cover_scope_update
BEFORE UPDATE OF cover_image_id ON music_album
FOR EACH ROW
WHEN new.cover_image_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'a library cover requires a LIBRARY image')
    WHERE (
        SELECT image.scope
        FROM image
        WHERE image.image_id = new.cover_image_id
    )
    = 'USER';

    UPDATE image
    SET scope = 'LIBRARY'
    WHERE image.image_id = new.cover_image_id AND image.scope IS NULL;
END;

CREATE TRIGGER tg_movie_cover_scope_insert
BEFORE INSERT ON movie
FOR EACH ROW
WHEN new.cover_image_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'a library cover requires a LIBRARY image')
    WHERE (
        SELECT image.scope
        FROM image
        WHERE image.image_id = new.cover_image_id
    )
    = 'USER';

    UPDATE image
    SET scope = 'LIBRARY'
    WHERE image.image_id = new.cover_image_id AND image.scope IS NULL;
END;

CREATE TRIGGER tg_movie_cover_scope_update
BEFORE UPDATE OF cover_image_id ON movie
FOR EACH ROW
WHEN new.cover_image_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'a library cover requires a LIBRARY image')
    WHERE (
        SELECT image.scope
        FROM image
        WHERE image.image_id = new.cover_image_id
    )
    = 'USER';

    UPDATE image
    SET scope = 'LIBRARY'
    WHERE image.image_id = new.cover_image_id AND image.scope IS NULL;
END;

CREATE TRIGGER tg_series_cover_scope_insert
BEFORE INSERT ON series
FOR EACH ROW
WHEN new.cover_image_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'a library cover requires a LIBRARY image')
    WHERE (
        SELECT image.scope
        FROM image
        WHERE image.image_id = new.cover_image_id
    )
    = 'USER';

    UPDATE image
    SET scope = 'LIBRARY'
    WHERE image.image_id = new.cover_image_id AND image.scope IS NULL;
END;

CREATE TRIGGER tg_series_cover_scope_update
BEFORE UPDATE OF cover_image_id ON series
FOR EACH ROW
WHEN new.cover_image_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'a library cover requires a LIBRARY image')
    WHERE (
        SELECT image.scope
        FROM image
        WHERE image.image_id = new.cover_image_id
    )
    = 'USER';

    UPDATE image
    SET scope = 'LIBRARY'
    WHERE image.image_id = new.cover_image_id AND image.scope IS NULL;
END;

CREATE TRIGGER tg_season_cover_scope_insert
BEFORE INSERT ON season
FOR EACH ROW
WHEN new.cover_image_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'a library cover requires a LIBRARY image')
    WHERE (
        SELECT image.scope
        FROM image
        WHERE image.image_id = new.cover_image_id
    )
    = 'USER';

    UPDATE image
    SET scope = 'LIBRARY'
    WHERE image.image_id = new.cover_image_id AND image.scope IS NULL;
END;

CREATE TRIGGER tg_season_cover_scope_update
BEFORE UPDATE OF cover_image_id ON season
FOR EACH ROW
WHEN new.cover_image_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'a library cover requires a LIBRARY image')
    WHERE (
        SELECT image.scope
        FROM image
        WHERE image.image_id = new.cover_image_id
    )
    = 'USER';

    UPDATE image
    SET scope = 'LIBRARY'
    WHERE image.image_id = new.cover_image_id AND image.scope IS NULL;
END;

-- A playlist cover and gallery media may only reference a USER asset, and a
-- USER image is owned by exactly one referrer.
CREATE TRIGGER tg_music_playlist_cover_scope_insert
BEFORE INSERT ON music_playlist
FOR EACH ROW
WHEN new.cover_image_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'a user cover requires a USER image')
    WHERE (
        SELECT image.scope
        FROM image
        WHERE image.image_id = new.cover_image_id
    )
    = 'LIBRARY';

    SELECT RAISE(ABORT, 'a USER image has exactly one referrer')
    WHERE EXISTS (
        SELECT 1
        FROM gallery_item
        WHERE gallery_item.image_id = new.cover_image_id
    );

    UPDATE image
    SET scope = 'USER'
    WHERE image.image_id = new.cover_image_id AND image.scope IS NULL;
END;

CREATE TRIGGER tg_music_playlist_cover_scope_update
BEFORE UPDATE OF cover_image_id ON music_playlist
FOR EACH ROW
WHEN new.cover_image_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'a user cover requires a USER image')
    WHERE (
        SELECT image.scope
        FROM image
        WHERE image.image_id = new.cover_image_id
    )
    = 'LIBRARY';

    SELECT RAISE(ABORT, 'a USER image has exactly one referrer')
    WHERE EXISTS (
        SELECT 1
        FROM gallery_item
        WHERE gallery_item.image_id = new.cover_image_id
    );

    UPDATE image
    SET scope = 'USER'
    WHERE image.image_id = new.cover_image_id AND image.scope IS NULL;
END;

CREATE TRIGGER tg_gallery_item_image_scope_insert
BEFORE INSERT ON gallery_item
FOR EACH ROW
WHEN new.image_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'gallery media requires a USER image')
    WHERE (
        SELECT image.scope
        FROM image
        WHERE image.image_id = new.image_id
    )
    = 'LIBRARY';

    SELECT RAISE(ABORT, 'a USER image has exactly one referrer')
    WHERE EXISTS (
        SELECT 1
        FROM music_playlist
        WHERE music_playlist.cover_image_id = new.image_id
    );

    UPDATE image
    SET scope = 'USER'
    WHERE image.image_id = new.image_id AND image.scope IS NULL;
END;

CREATE TRIGGER tg_gallery_item_image_scope_update
BEFORE UPDATE OF image_id ON gallery_item
FOR EACH ROW
WHEN new.image_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'gallery media requires a USER image')
    WHERE (
        SELECT image.scope
        FROM image
        WHERE image.image_id = new.image_id
    )
    = 'LIBRARY';

    SELECT RAISE(ABORT, 'a USER image has exactly one referrer')
    WHERE EXISTS (
        SELECT 1
        FROM music_playlist
        WHERE music_playlist.cover_image_id = new.image_id
    );

    UPDATE image
    SET scope = 'USER'
    WHERE image.image_id = new.image_id AND image.scope IS NULL;
END;

CREATE TRIGGER tg_gallery_item_video_scope_insert
BEFORE INSERT ON gallery_item
FOR EACH ROW
WHEN new.video_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'gallery media requires a USER video')
    WHERE (
        SELECT video.scope
        FROM video
        WHERE video.video_id = new.video_id
    )
    = 'LIBRARY';

    UPDATE video
    SET scope = 'USER'
    WHERE video.video_id = new.video_id AND video.scope IS NULL;
END;

CREATE TRIGGER tg_gallery_item_video_scope_update
BEFORE UPDATE OF video_id ON gallery_item
FOR EACH ROW
WHEN new.video_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'gallery media requires a USER video')
    WHERE (
        SELECT video.scope
        FROM video
        WHERE video.video_id = new.video_id
    )
    = 'LIBRARY';

    UPDATE video
    SET scope = 'USER'
    WHERE video.video_id = new.video_id AND video.scope IS NULL;
END;

-- A USER image belongs to one playlist cover at most; the trigger above rejects
-- a referrer in another table, this index rejects a second playlist.
CREATE UNIQUE INDEX uq_music_playlist_cover_image_id ON music_playlist (
    cover_image_id
)
WHERE cover_image_id IS NOT NULL;

-- The first attachment fixes the scope; it never changes afterwards.
CREATE TRIGGER tg_image_scope_is_immutable
BEFORE UPDATE OF scope ON image
FOR EACH ROW
WHEN old.scope IS NOT NULL AND new.scope IS NOT old.scope
BEGIN
    SELECT RAISE(ABORT, 'an asset scope is immutable once set');
END;

CREATE TRIGGER tg_video_scope_is_immutable
BEFORE UPDATE OF scope ON video
FOR EACH ROW
WHEN old.scope IS NOT NULL AND new.scope IS NOT old.scope
BEGIN
    SELECT RAISE(ABORT, 'an asset scope is immutable once set');
END;
