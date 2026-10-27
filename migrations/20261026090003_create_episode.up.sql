CREATE TABLE episode (
    episode_id INTEGER NOT NULL,
    season_id INTEGER NOT NULL,
    episode_index INTEGER NOT NULL,
    name TEXT NOT NULL,
    plot TEXT NOT NULL,
    air_date DATE NOT NULL,
    video_id INTEGER NOT NULL,
    CONSTRAINT pk_episode_episode_id PRIMARY KEY (episode_id),
    CONSTRAINT uq_episode_season_id_episode_index UNIQUE (
        season_id,
        episode_index
    ),
    CONSTRAINT uq_episode_video_id UNIQUE (video_id),
    CONSTRAINT chk_episode_episode_index CHECK (
        episode_index BETWEEN 0 AND 4294967295
    ),
    CONSTRAINT fk_episode_season FOREIGN KEY (season_id) REFERENCES season (
        season_id
    ) ON DELETE CASCADE,
    CONSTRAINT fk_episode_video FOREIGN KEY (video_id) REFERENCES video (
        video_id
    )
);
