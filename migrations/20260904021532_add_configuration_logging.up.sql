ALTER TABLE configuration
ADD COLUMN is_log_ansi INTEGER NOT NULL DEFAULT 0
CONSTRAINT chk_configuration_is_log_ansi CHECK (is_log_ansi IN (0, 1));

ALTER TABLE configuration
ADD COLUMN log_level TEXT NOT NULL DEFAULT 'debug,sqlx=warn';

ALTER TABLE configuration
ADD COLUMN log_max_files INTEGER NOT NULL DEFAULT 7
CONSTRAINT chk_configuration_log_max_files CHECK (log_max_files >= 0);

ALTER TABLE configuration
ADD COLUMN log_rotation TEXT NOT NULL DEFAULT 'DAILY'
CONSTRAINT chk_configuration_log_rotation CHECK (
    log_rotation IN ('MINUTELY', 'HOURLY', 'DAILY', 'NEVER')
);
