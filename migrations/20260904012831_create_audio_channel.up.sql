CREATE TABLE audio_channel (
    audio_channel_id TEXT NOT NULL,
    nb_channel INTEGER NOT NULL,
    description TEXT NOT NULL,
    CONSTRAINT pk_audio_channel_audio_channel_id PRIMARY KEY (audio_channel_id)
);
