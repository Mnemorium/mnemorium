CREATE TABLE stream (
    stream_id INTEGER NOT NULL,
    stream_index INTEGER NOT NULL,
    name TEXT NOT NULL,
    is_default BOOLEAN NOT NULL DEFAULT 0,
    video_id INTEGER NOT NULL,
    CONSTRAINT pk_stream_stream_id PRIMARY KEY (stream_id),
    CONSTRAINT fk_stream_video FOREIGN KEY (video_id) REFERENCES video (
        video_id
    ),
    CONSTRAINT chk_stream_is_default CHECK (is_default IN (0, 1))
);
