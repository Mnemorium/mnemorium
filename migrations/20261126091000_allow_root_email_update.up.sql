-- The root's username, role and credential are fixed (`REQ-USER-021`), but the
-- account maintains its own email. Restrict the guard to the identity fields so
-- an email-only update reaches the ordinary path.
DROP TRIGGER tg_user_update_root_admin;

CREATE TRIGGER tg_user_update_root_admin
BEFORE UPDATE ON user
FOR EACH ROW
WHEN
    old.user_id = 0
    AND (
        new.username <> old.username
        OR new.role <> old.role
        OR new.credential_id <> old.credential_id
    )
BEGIN
    SELECT RAISE(ABORT, 'cannot modify root admin');
END;
