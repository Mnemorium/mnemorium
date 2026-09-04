CREATE TABLE audio (
    audio_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    duration_ms REAL NOT NULL,
    codec TEXT NOT NULL,
    sample_rate_hz REAL NOT NULL,
    audio_channel_id TEXT NOT NULL,
    bit_depth INTEGER NOT NULL,
    block_size INTEGER NOT NULL,
    file_id INTEGER NOT NULL,
    CONSTRAINT pk_audio_audio_id PRIMARY KEY (audio_id),
    CONSTRAINT uq_audio_file_id UNIQUE (file_id),
    CONSTRAINT fk_audio_audio_channel FOREIGN KEY (
        audio_channel_id
    ) REFERENCES audio_channel (audio_channel_id),
    CONSTRAINT fk_audio_file FOREIGN KEY (file_id) REFERENCES file (file_id),
    CONSTRAINT chk_audio_duration_ms CHECK (duration_ms >= 0),
    CONSTRAINT chk_audio_sample_rate_hz CHECK (sample_rate_hz >= 0),
    CONSTRAINT chk_audio_bit_depth CHECK (
        bit_depth > 0 AND (bit_depth & (bit_depth - 1)) = 0
    ),
    CONSTRAINT chk_audio_block_size CHECK (
        block_size > 0 AND (block_size & (block_size - 1)) = 0
    )
);
