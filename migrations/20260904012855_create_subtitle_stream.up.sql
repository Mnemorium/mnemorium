CREATE TABLE subtitle_stream (
    stream_id INTEGER NOT NULL,
    is_forced BOOLEAN NOT NULL DEFAULT 0,
    language_id INTEGER NOT NULL,
    CONSTRAINT pk_subtitle_stream_stream_id PRIMARY KEY (stream_id),
    CONSTRAINT fk_subtitle_stream_stream FOREIGN KEY (
        stream_id
    ) REFERENCES stream (stream_id),
    CONSTRAINT fk_subtitle_stream_language FOREIGN KEY (
        language_id
    ) REFERENCES language (language_id),
    CONSTRAINT chk_subtitle_stream_is_forced CHECK (is_forced IN (0, 1))
);
