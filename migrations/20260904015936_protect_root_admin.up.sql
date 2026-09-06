CREATE TRIGGER tg_user_delete_root_admin
BEFORE DELETE ON user
FOR EACH ROW
WHEN old.user_id = 0
BEGIN
    SELECT RAISE(ABORT, 'cannot delete root admin');
END;

CREATE TRIGGER tg_user_update_root_admin
BEFORE UPDATE ON user
FOR EACH ROW
WHEN old.user_id = 0
BEGIN
    SELECT RAISE(ABORT, 'cannot modify root admin');
END;
