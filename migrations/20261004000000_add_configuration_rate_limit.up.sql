ALTER TABLE configuration
ADD COLUMN rate_limit_trusted_proxies TEXT NOT NULL DEFAULT '';

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
