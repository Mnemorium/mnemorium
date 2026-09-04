CREATE TABLE person (
    person_id INTEGER NOT NULL,
    given_name VARCHAR(50) NOT NULL,
    family_name VARCHAR(50),
    birth_date DATE,
    CONSTRAINT pk_person_person_id PRIMARY KEY (person_id)
);
