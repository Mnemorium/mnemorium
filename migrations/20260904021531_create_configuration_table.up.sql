CREATE TABLE configuration (
    configuration_id INTEGER NOT NULL,
    jwt_secret TEXT NOT NULL,
    jwt_ttl INTEGER NOT NULL,
    pepper TEXT NOT NULL,
    sqlite3_path TEXT NOT NULL,
    sqlite3_max_connections INTEGER NOT NULL,
    CONSTRAINT pk_configuration_configuration_id PRIMARY KEY (configuration_id),
    CONSTRAINT chk_configuration_configuration_id CHECK (configuration_id = 0),
    CONSTRAINT chk_configuration_jwt_secret CHECK (LENGTH(jwt_secret) = 64),
    CONSTRAINT chk_configuration_jwt_ttl CHECK (jwt_ttl > 0),
    CONSTRAINT chk_configuration_pepper CHECK (LENGTH(pepper) = 64),
    CONSTRAINT chk_configuration_sqlite3_max_connections CHECK (
        sqlite3_max_connections > 0
    )
);

CREATE TRIGGER tg_configuration_delete_row
BEFORE DELETE ON configuration
FOR EACH ROW
WHEN old.configuration_id = 0
BEGIN
    SELECT RAISE(ABORT, 'cannot delete configuration row');
END;
