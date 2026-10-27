CREATE TRIGGER tg_season_cover_is_image_insert
BEFORE INSERT ON season
FOR EACH ROW
WHEN
    new.cover_file_id IS NOT NULL
    AND (
        SELECT mime_type.type
        FROM mime_type
        WHERE
            mime_type.mime_type_id = (
                SELECT file.mime_type_id
                FROM file
                WHERE file.file_id = new.cover_file_id
            )
    )
    != 'IMAGE'
BEGIN
    SELECT RAISE(ABORT, 'cover_file_id must reference an image file');
END;

CREATE TRIGGER tg_season_cover_is_image_update
BEFORE UPDATE OF cover_file_id ON season
FOR EACH ROW
WHEN
    new.cover_file_id IS NOT NULL
    AND (
        SELECT mime_type.type
        FROM mime_type
        WHERE
            mime_type.mime_type_id = (
                SELECT file.mime_type_id
                FROM file
                WHERE file.file_id = new.cover_file_id
            )
    )
    != 'IMAGE'
BEGIN
    SELECT RAISE(ABORT, 'cover_file_id must reference an image file');
END;
