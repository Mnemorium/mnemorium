CREATE TRIGGER tg_audio_upper_codec_insert
AFTER INSERT ON audio
FOR EACH ROW
WHEN new.codec != upper(new.codec)
BEGIN
    UPDATE audio SET codec = upper(codec)
    WHERE rowid = new.rowid;
END;

CREATE TRIGGER tg_audio_upper_codec_update
AFTER UPDATE OF codec ON audio
FOR EACH ROW
WHEN new.codec != upper(new.codec)
BEGIN
    UPDATE audio SET codec = upper(codec)
    WHERE rowid = old.rowid;
END;
