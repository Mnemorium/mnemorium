-- The entity flag and the resource visibility are one fact: the
-- representation and the listing filter read `gallery.is_public` and
-- `music_playlist.is_public`, while every decision reads
-- `resource.visibility`. A visibility change made through the permission API
-- SHALL move both (`REQ-AUTHORIZATION-016`).
--
-- The seeded default gallery (`gallery_id = 0`) keeps its immutable PUBLIC
-- flag: `tg_gallery_update_default_gallery` refuses any change to the seeded
-- row, so the sync skips it rather than aborting the transaction.
CREATE TRIGGER tg_resource_visibility_gallery
AFTER UPDATE OF visibility ON resource
FOR EACH ROW
WHEN new.kind = 'GALLERY' AND new.entity_id <> 0
BEGIN
    UPDATE gallery
    SET is_public = (new.visibility = 'PUBLIC')
    WHERE gallery_id = new.entity_id;
END;

CREATE TRIGGER tg_resource_visibility_music_playlist
AFTER UPDATE OF visibility ON resource
FOR EACH ROW
WHEN new.kind = 'MUSIC_PLAYLIST'
BEGIN
    UPDATE music_playlist
    SET is_public = (new.visibility = 'PUBLIC')
    WHERE music_playlist_id = new.entity_id;
END;
