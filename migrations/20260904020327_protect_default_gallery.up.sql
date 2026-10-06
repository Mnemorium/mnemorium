INSERT INTO gallery (
    gallery_id,
    name,
    created_at,
    last_modified_at,
    is_public,
    user_id
)
VALUES (0, 'Default', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 1, NULL);

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
        old.gallery_id IS NOT new.gallery_id
        OR old.name != new.name
        OR old.created_at != new.created_at
        OR old.is_public != new.is_public
        OR old.user_id IS NOT new.user_id
    )
BEGIN
    SELECT RAISE(ABORT, 'cannot modify default gallery');
END;
