CREATE TRIGGER tg_movie_upper_country_of_origin_insert
AFTER INSERT ON movie
FOR EACH ROW
WHEN new.country_of_origin != upper(new.country_of_origin)
BEGIN
    UPDATE movie SET country_of_origin = upper(country_of_origin)
    WHERE rowid = new.rowid;
END;

CREATE TRIGGER tg_movie_upper_country_of_origin_update
AFTER UPDATE OF country_of_origin ON movie
FOR EACH ROW
WHEN new.country_of_origin != upper(new.country_of_origin)
BEGIN
    UPDATE movie SET country_of_origin = upper(country_of_origin)
    WHERE rowid = old.rowid;
END;
