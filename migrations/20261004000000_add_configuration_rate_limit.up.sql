ALTER TABLE configuration
ADD COLUMN is_behind_proxy INTEGER NOT NULL DEFAULT 0
CONSTRAINT chk_configuration_is_behind_proxy CHECK (is_behind_proxy IN (0, 1));

ALTER TABLE configuration
ADD COLUMN rate_limit_burst_size INTEGER NOT NULL DEFAULT 5
CONSTRAINT chk_configuration_rate_limit_burst_size CHECK (
    rate_limit_burst_size > 0
);

ALTER TABLE configuration
ADD COLUMN rate_limit_period_seconds INTEGER NOT NULL DEFAULT 12
CONSTRAINT chk_configuration_rate_limit_period_seconds CHECK (
    rate_limit_period_seconds > 0
);
