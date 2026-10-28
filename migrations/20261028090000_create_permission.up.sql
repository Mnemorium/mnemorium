CREATE TABLE permission (
    permission_id INTEGER,
    user_id INTEGER NOT NULL,
    action_name TEXT NOT NULL,
    scope TEXT NOT NULL,
    CONSTRAINT pk_permission_permission_id PRIMARY KEY (permission_id),
    CONSTRAINT uq_permission_user_action UNIQUE (user_id, action_name),
    CONSTRAINT fk_permission_user FOREIGN KEY (
        user_id
    ) REFERENCES user (user_id),
    CONSTRAINT chk_permission_scope CHECK (
        scope IN ('ALL', 'PUBLIC_OR_OWNED', 'OWNED', 'LISTED', 'NONE')
    )
);

CREATE TABLE permission_resource (
    permission_id INTEGER NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id INTEGER NOT NULL,
    CONSTRAINT pk_permission_resource PRIMARY KEY (
        permission_id,
        resource_type,
        resource_id
    ),
    CONSTRAINT fk_permission_resource_permission FOREIGN KEY (
        permission_id
    ) REFERENCES permission (permission_id) ON DELETE CASCADE
);

CREATE TRIGGER tg_permission_insert_root_admin
BEFORE INSERT ON permission
FOR EACH ROW
WHEN new.user_id = 0
BEGIN
    SELECT RAISE(ABORT, 'cannot grant a permission to the root admin');
END;

CREATE TRIGGER tg_permission_update_root_admin
BEFORE UPDATE ON permission
FOR EACH ROW
WHEN new.user_id = 0
BEGIN
    SELECT RAISE(ABORT, 'cannot grant a permission to the root admin');
END;
