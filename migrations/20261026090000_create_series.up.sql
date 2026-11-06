CREATE TABLE series (
    series_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    plot TEXT NOT NULL,
    country_of_origin VARCHAR(30),
    first_air_date DATE,
    cover_image_id INTEGER,
    CONSTRAINT pk_series_series_id PRIMARY KEY (series_id),
    CONSTRAINT fk_series_cover_image FOREIGN KEY (
        cover_image_id
    ) REFERENCES image (image_id)
);
