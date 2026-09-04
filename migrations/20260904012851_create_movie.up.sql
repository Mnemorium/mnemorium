CREATE TABLE movie (
    movie_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    plot TEXT NOT NULL,
    country_of_origin VARCHAR(30) NOT NULL,
    release_date DATE NOT NULL DEFAULT (date('now')),
    video_id INTEGER NOT NULL,
    CONSTRAINT pk_movie_movie_id PRIMARY KEY (movie_id),
    CONSTRAINT uq_movie_video_id UNIQUE (video_id),
    CONSTRAINT fk_movie_video FOREIGN KEY (video_id) REFERENCES video (video_id)
);
