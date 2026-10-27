CREATE TABLE season (
    season_id INTEGER NOT NULL,
    series_id INTEGER NOT NULL,
    season_index INTEGER NOT NULL DEFAULT 0,
    name TEXT,
    cover_file_id INTEGER,
    CONSTRAINT pk_season_season_id PRIMARY KEY (season_id),
    CONSTRAINT uq_season_series_id_season_index UNIQUE (
        series_id,
        season_index
    ),
    CONSTRAINT chk_season_season_index CHECK (
        season_index BETWEEN 0 AND 4294967295
    ),
    CONSTRAINT fk_season_series FOREIGN KEY (series_id) REFERENCES series (
        series_id
    ) ON DELETE CASCADE,
    CONSTRAINT fk_season_cover_file FOREIGN KEY (
        cover_file_id
    ) REFERENCES file (file_id)
);
