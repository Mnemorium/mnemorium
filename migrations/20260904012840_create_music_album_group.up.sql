CREATE TABLE music_album_group (
    music_album_id INTEGER NOT NULL,
    music_group_id INTEGER NOT NULL,
    CONSTRAINT pk_music_album_group_album_group PRIMARY KEY (
        music_album_id, music_group_id
    ),
    CONSTRAINT fk_music_album_group_album FOREIGN KEY (
        music_album_id
    ) REFERENCES music_album (music_album_id),
    CONSTRAINT fk_music_album_group_group FOREIGN KEY (
        music_group_id
    ) REFERENCES music_group (music_group_id)
);
