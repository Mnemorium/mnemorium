DELETE FROM music_album_genre
WHERE music_album_id = 0;
DELETE FROM music_medium
WHERE music_medium_id = 0;
DELETE FROM music_album
WHERE music_album_id = 0;
DELETE FROM music_group
WHERE music_group_id = 0;
DELETE FROM genre
WHERE genre_id = 'UNKNOWN';
