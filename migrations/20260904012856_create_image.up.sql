CREATE TABLE image (
    image_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    width_px INTEGER NOT NULL,
    height_px INTEGER NOT NULL,
    orientation VARCHAR(20) NOT NULL,
    created_at DATE NOT NULL,
    color_id TEXT NOT NULL,
    CONSTRAINT pk_image_image_id PRIMARY KEY (image_id),
    CONSTRAINT fk_image_color FOREIGN KEY (color_id) REFERENCES color (
        color_id
    ),
    CONSTRAINT chk_image_width_px CHECK (width_px > 0),
    CONSTRAINT chk_image_height_px CHECK (height_px > 0),
    CONSTRAINT chk_image_orientation CHECK (
        orientation IN ('LANDSCAPE', 'PORTRAIT', 'SQUARE')
    )
);
