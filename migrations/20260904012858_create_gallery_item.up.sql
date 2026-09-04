CREATE TABLE gallery_item (
    gallery_id INTEGER NOT NULL,
    image_id INTEGER NOT NULL,
    item_index INTEGER NOT NULL,
    added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT pk_gallery_item_gallery_image_index PRIMARY KEY (
        gallery_id, image_id, item_index
    ),
    CONSTRAINT fk_gallery_item_gallery FOREIGN KEY (
        gallery_id
    ) REFERENCES gallery (gallery_id),
    CONSTRAINT fk_gallery_item_image FOREIGN KEY (image_id) REFERENCES image (
        image_id
    )
);
