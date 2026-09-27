ALTER TABLE configuration
ADD COLUMN asset_storage_root TEXT NOT NULL DEFAULT 'data'
CONSTRAINT chk_configuration_asset_storage_root CHECK (
    length(asset_storage_root) > 0
);

ALTER TABLE configuration
ADD COLUMN asset_upload_chunk_size_bytes INTEGER NOT NULL DEFAULT 5242880
CONSTRAINT chk_configuration_asset_upload_chunk_size_bytes CHECK (
    asset_upload_chunk_size_bytes > 0
);

ALTER TABLE configuration
ADD COLUMN asset_upload_expiry_seconds INTEGER NOT NULL DEFAULT 86400
CONSTRAINT chk_configuration_asset_upload_expiry_seconds CHECK (
    asset_upload_expiry_seconds > 0
);
