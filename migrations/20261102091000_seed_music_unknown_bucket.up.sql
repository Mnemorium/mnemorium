INSERT INTO genre (genre_id) VALUES ('UNKNOWN');

INSERT INTO music_group (music_group_id, stage_name) VALUES (0, 'Unknown');

INSERT INTO music_album (music_album_id, name, release_date, music_group_id)
VALUES (0, 'Unknown', NULL, 0);

INSERT INTO music_medium (music_medium_id, music_album_id, type)
VALUES (0, 0, 'DIGITAL');

INSERT INTO music_album_genre (music_album_id, genre_id) VALUES (0, 'UNKNOWN');
