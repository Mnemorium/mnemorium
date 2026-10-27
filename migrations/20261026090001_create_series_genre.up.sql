CREATE TABLE series_genre (
    series_id INTEGER NOT NULL,
    genre_id TEXT NOT NULL,
    CONSTRAINT pk_series_genre_series_genre PRIMARY KEY (series_id, genre_id),
    CONSTRAINT fk_series_genre_series FOREIGN KEY (
        series_id
    ) REFERENCES series (
        series_id
    ),
    CONSTRAINT fk_series_genre_genre FOREIGN KEY (genre_id) REFERENCES genre (
        genre_id
    )
);
