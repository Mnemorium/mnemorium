CREATE TABLE user (
    user_id INTEGER NOT NULL,
    role VARCHAR(50) NOT NULL,
    username VARCHAR(100) NOT NULL,
    email TEXT,
    credential_id INTEGER NOT NULL,
    CONSTRAINT pk_user_user_id PRIMARY KEY (user_id),
    CONSTRAINT uq_user_username UNIQUE (username),
    CONSTRAINT uq_user_email UNIQUE (email),
    CONSTRAINT uq_user_credential_id UNIQUE (credential_id),
    CONSTRAINT fk_user_credential FOREIGN KEY (
        credential_id
    ) REFERENCES credential (credential_id),
    CONSTRAINT chk_user_role CHECK (role IN ('ADMIN', 'STANDARD')),
    CONSTRAINT chk_user_username CHECK (length(username) >= 5),
    CONSTRAINT chk_user_email CHECK (email LIKE '%_@_%._%')
);
