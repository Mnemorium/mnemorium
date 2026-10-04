CREATE TRIGGER tg_genre_upper_genre_id_insert
AFTER INSERT ON genre
FOR EACH ROW
WHEN new.genre_id != upper(new.genre_id)
BEGIN
    UPDATE genre SET genre_id = upper(genre_id)
    WHERE rowid = new.rowid;
END;
