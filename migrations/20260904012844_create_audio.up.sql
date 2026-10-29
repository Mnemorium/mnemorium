CREATE TABLE audio (
    audio_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    duration_ms REAL NOT NULL,
    codec TEXT NOT NULL,
    sample_rate_hz REAL NOT NULL,
    bit_depth INTEGER,
    block_size INTEGER,
    file_id INTEGER NOT NULL,
    CONSTRAINT pk_audio_audio_id PRIMARY KEY (audio_id),
    CONSTRAINT uq_audio_file_id UNIQUE (file_id),
    CONSTRAINT fk_audio_file FOREIGN KEY (file_id) REFERENCES file (file_id),
    CONSTRAINT chk_audio_duration_ms CHECK (duration_ms >= 0),
    CONSTRAINT chk_audio_sample_rate_hz CHECK (sample_rate_hz >= 0),
    CONSTRAINT chk_audio_bit_depth CHECK (bit_depth IS NULL OR bit_depth > 0),
    CONSTRAINT chk_audio_block_size CHECK (block_size IS NULL OR block_size > 0)
);
