CREATE TRIGGER tg_gallery_delete_default_gallery
BEFORE DELETE ON gallery
FOR EACH ROW
WHEN old.gallery_id = 0
BEGIN
    SELECT RAISE(ABORT, 'cannot delete default gallery');
END;

CREATE TRIGGER tg_gallery_update_default_gallery
BEFORE UPDATE ON gallery
FOR EACH ROW
WHEN
    old.gallery_id = 0
    AND (
        old.name != new.name
        OR old.created_at != new.created_at
        OR old.is_public != new.is_public
    )
BEGIN
    SELECT RAISE(ABORT, 'cannot modify default gallery');
END;
