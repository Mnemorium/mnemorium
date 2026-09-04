CREATE TABLE audio_stream (
    stream_id INTEGER NOT NULL,
    is_commentary BOOLEAN NOT NULL DEFAULT 0,
    audio_id INTEGER NOT NULL,
    language_id INTEGER NOT NULL,
    CONSTRAINT pk_audio_stream_stream_id PRIMARY KEY (stream_id),
    CONSTRAINT fk_audio_stream_stream FOREIGN KEY (
        stream_id
    ) REFERENCES stream (stream_id),
    CONSTRAINT fk_audio_stream_audio FOREIGN KEY (audio_id) REFERENCES audio (
        audio_id
    ),
    CONSTRAINT fk_audio_stream_language FOREIGN KEY (
        language_id
    ) REFERENCES language (language_id),
    CONSTRAINT chk_audio_stream_is_commentary CHECK (is_commentary IN (0, 1))
);
