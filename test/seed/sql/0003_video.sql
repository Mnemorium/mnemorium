-- Seeded video library: one film and one show with a season and an episode.
--
-- Image 9001 is already LIBRARY from the music group cover; the film and the
-- show share it as their cover.

INSERT INTO movie (
    movie_id,
    name,
    plot,
    country_of_origin,
    release_date,
    video_id,
    cover_image_id
)
VALUES (9001, 'Seed Movie', 'A seeded film.', 'US', '2020-01-01', 9001, 9001);

INSERT INTO movie_genre (movie_id, genre_id) VALUES (9001, 'DRAMA');

INSERT INTO series (
    series_id, name, plot, country_of_origin, first_air_date, cover_image_id
)
VALUES (9001, 'Seed Show', 'A seeded show.', 'US', '2020-01-01', 9001);

INSERT INTO series_genre (series_id, genre_id) VALUES (9001, 'DRAMA');

INSERT INTO season (season_id, series_id, season_index, name, cover_image_id)
VALUES (9001, 9001, 1, 'Season One', 9001);

INSERT INTO episode (
    episode_id, season_id, episode_index, name, plot, air_date, video_id
)
VALUES (
    9001, 9001, 1, 'Pilot', 'The first seeded episode.', '2020-01-02', 9002
);
