CREATE TRIGGER tg_movie_reject_episode_video_insert
BEFORE INSERT ON movie
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'a video must back at most one film or episode')
    WHERE EXISTS (
        SELECT 1
        FROM episode
        WHERE episode.video_id = new.video_id
    );
END;

CREATE TRIGGER tg_movie_reject_episode_video_update
BEFORE UPDATE OF video_id ON movie
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'a video must back at most one film or episode')
    WHERE EXISTS (
        SELECT 1
        FROM episode
        WHERE episode.video_id = new.video_id
    );
END;

CREATE TRIGGER tg_episode_reject_movie_video_insert
BEFORE INSERT ON episode
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'a video must back at most one film or episode')
    WHERE EXISTS (
        SELECT 1
        FROM movie
        WHERE movie.video_id = new.video_id
    );
END;

CREATE TRIGGER tg_episode_reject_movie_video_update
BEFORE UPDATE OF video_id ON episode
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'a video must back at most one film or episode')
    WHERE EXISTS (
        SELECT 1
        FROM movie
        WHERE movie.video_id = new.video_id
    );
END;
