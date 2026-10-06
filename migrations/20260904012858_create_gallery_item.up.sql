CREATE TABLE gallery_item (
    gallery_item_id INTEGER NOT NULL,
    gallery_id INTEGER NOT NULL,
    image_id INTEGER,
    video_id INTEGER,
    item_index INTEGER NOT NULL,
    added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT pk_gallery_item_gallery_item_id PRIMARY KEY (gallery_item_id),
    CONSTRAINT fk_gallery_item_gallery FOREIGN KEY (
        gallery_id
    ) REFERENCES gallery (gallery_id) ON DELETE CASCADE,
    CONSTRAINT fk_gallery_item_image FOREIGN KEY (
        image_id
    ) REFERENCES image (image_id) ON DELETE CASCADE,
    CONSTRAINT fk_gallery_item_video FOREIGN KEY (
        video_id
    ) REFERENCES video (video_id) ON DELETE CASCADE,
    CONSTRAINT uq_gallery_item_gallery_id_item_index UNIQUE (
        gallery_id,
        item_index
    ),
    CONSTRAINT chk_gallery_item_one_media CHECK (
        (image_id IS NOT NULL AND video_id IS NULL)
        OR (image_id IS NULL AND video_id IS NOT NULL)
    ),
    CONSTRAINT chk_gallery_item_item_index CHECK (item_index >= 0)
);

CREATE UNIQUE INDEX uq_gallery_item_image_id ON gallery_item (image_id)
WHERE image_id IS NOT NULL;

CREATE UNIQUE INDEX uq_gallery_item_video_id ON gallery_item (video_id)
WHERE video_id IS NOT NULL;
