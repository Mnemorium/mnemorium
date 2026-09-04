CREATE TABLE configuration (
    configuration_id INTEGER NOT NULL,
    content TEXT NOT NULL,
    CONSTRAINT pk_configuration_configuration_id PRIMARY KEY (configuration_id),
    CONSTRAINT chk_configuration_configuration_id CHECK (configuration_id = 0)
);

INSERT INTO configuration (configuration_id, content) VALUES (0, '{}');

CREATE TRIGGER tg_configuration_delete_row
BEFORE DELETE ON configuration
FOR EACH ROW
WHEN old.configuration_id = 0
BEGIN
    SELECT RAISE(ABORT, 'cannot delete configuration row');
END;
