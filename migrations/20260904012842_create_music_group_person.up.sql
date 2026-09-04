CREATE TABLE music_group_person (
    music_group_id INTEGER NOT NULL,
    person_id INTEGER NOT NULL,
    CONSTRAINT pk_music_group_person_group_person PRIMARY KEY (
        music_group_id, person_id
    ),
    CONSTRAINT fk_music_group_person_group FOREIGN KEY (
        music_group_id
    ) REFERENCES music_group (music_group_id),
    CONSTRAINT fk_music_group_person_person FOREIGN KEY (
        person_id
    ) REFERENCES person (person_id)
);
