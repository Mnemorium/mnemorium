CREATE TABLE movie_genre (
    movie_id INTEGER NOT NULL,
    genre_id TEXT NOT NULL,
    CONSTRAINT pk_movie_genre_movie_genre PRIMARY KEY (movie_id, genre_id),
    CONSTRAINT fk_movie_genre_movie FOREIGN KEY (movie_id) REFERENCES movie (
        movie_id
    ),
    CONSTRAINT fk_movie_genre_genre FOREIGN KEY (genre_id) REFERENCES genre (
        genre_id
    )
);
