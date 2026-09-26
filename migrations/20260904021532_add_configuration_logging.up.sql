ALTER TABLE configuration
ADD COLUMN log_ansi INTEGER NOT NULL DEFAULT 0 CHECK (
    log_ansi IN (0, 1)
);

ALTER TABLE configuration
ADD COLUMN log_level TEXT NOT NULL DEFAULT 'debug,sqlx=warn';

ALTER TABLE configuration
ADD COLUMN log_max_files INTEGER NOT NULL DEFAULT 7 CHECK (
    log_max_files >= 0
);

ALTER TABLE configuration
ADD COLUMN log_rotation TEXT NOT NULL DEFAULT 'DAILY' CHECK (
    log_rotation IN ('MINUTELY', 'HOURLY', 'DAILY', 'NEVER')
);
