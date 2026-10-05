ALTER TABLE configuration DROP COLUMN rate_limit_period_seconds;

ALTER TABLE configuration DROP COLUMN rate_limit_client_ip_header;

ALTER TABLE configuration DROP COLUMN rate_limit_burst_size;

ALTER TABLE configuration DROP COLUMN rate_limit_trusted_proxies;
