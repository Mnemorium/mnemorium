CREATE TABLE language (
    language_id INTEGER NOT NULL,
    english_name VARCHAR(50) NOT NULL,
    native_name VARCHAR(50) NOT NULL,
    code VARCHAR(2) NOT NULL,
    CONSTRAINT pk_language_language_id PRIMARY KEY (language_id),
    CONSTRAINT uq_language_english_name UNIQUE (english_name),
    CONSTRAINT uq_language_native_name UNIQUE (native_name),
    CONSTRAINT uq_language_code UNIQUE (code)
);
