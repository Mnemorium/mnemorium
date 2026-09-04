CREATE TABLE video (
    video_id INTEGER NOT NULL,
    duration_ms REAL NOT NULL,
    codec VARCHAR(20) NOT NULL,
    frame_count INTEGER NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    color_id TEXT NOT NULL,
    scan_type VARCHAR(20) NOT NULL,
    file_id INTEGER NOT NULL,
    CONSTRAINT pk_video_video_id PRIMARY KEY (video_id),
    CONSTRAINT uq_video_file_id UNIQUE (file_id),
    CONSTRAINT fk_video_color FOREIGN KEY (color_id) REFERENCES color (
        color_id
    ),
    CONSTRAINT fk_video_file FOREIGN KEY (file_id) REFERENCES file (file_id),
    CONSTRAINT chk_video_frame_count CHECK (frame_count > 0),
    CONSTRAINT chk_video_width CHECK (width > 0),
    CONSTRAINT chk_video_height CHECK (height > 0),
    CONSTRAINT chk_video_scan_type CHECK (
        scan_type IN ('PROGRESSIVE', 'INTERLACED', 'MBAFF', 'PAFF')
    )
);
