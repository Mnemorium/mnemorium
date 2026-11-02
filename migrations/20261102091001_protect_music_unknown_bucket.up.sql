CREATE TRIGGER tg_music_group_delete_unknown
BEFORE DELETE ON music_group
FOR EACH ROW
WHEN old.music_group_id = 0
BEGIN
    SELECT RAISE(ABORT, 'cannot delete the unknown music group');
END;

CREATE TRIGGER tg_music_group_update_unknown
BEFORE UPDATE ON music_group
FOR EACH ROW
WHEN old.music_group_id = 0
BEGIN
    SELECT RAISE(ABORT, 'cannot modify the unknown music group');
END;

CREATE TRIGGER tg_music_album_delete_unknown
BEFORE DELETE ON music_album
FOR EACH ROW
WHEN old.music_album_id = 0
BEGIN
    SELECT RAISE(ABORT, 'cannot delete the unknown music album');
END;

CREATE TRIGGER tg_music_album_update_unknown
BEFORE UPDATE ON music_album
FOR EACH ROW
WHEN old.music_album_id = 0
BEGIN
    SELECT RAISE(ABORT, 'cannot modify the unknown music album');
END;

CREATE TRIGGER tg_music_medium_delete_unknown
BEFORE DELETE ON music_medium
FOR EACH ROW
WHEN old.music_medium_id = 0
BEGIN
    SELECT RAISE(ABORT, 'cannot delete the unknown bucket medium');
END;

CREATE TRIGGER tg_music_medium_update_unknown
BEFORE UPDATE ON music_medium
FOR EACH ROW
WHEN old.music_medium_id = 0
BEGIN
    SELECT RAISE(ABORT, 'cannot modify the unknown bucket medium');
END;
