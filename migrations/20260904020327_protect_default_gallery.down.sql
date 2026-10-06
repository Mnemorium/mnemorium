DROP TRIGGER tg_gallery_delete_default_gallery;
DROP TRIGGER tg_gallery_update_default_gallery;
DELETE FROM gallery
WHERE gallery_id = 0;
