CREATE TABLE series (
    series_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    plot TEXT NOT NULL,
    country_of_origin VARCHAR(30) NOT NULL,
    first_air_date DATE NOT NULL DEFAULT (date('now')),
    cover_file_id INTEGER,
    CONSTRAINT pk_series_series_id PRIMARY KEY (series_id),
    CONSTRAINT fk_series_cover_file FOREIGN KEY (
        cover_file_id
    ) REFERENCES file (file_id)
);
