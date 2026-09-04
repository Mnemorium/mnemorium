CREATE TABLE gallery_video (
    gallery_id INTEGER NOT NULL,
    video_id INTEGER NOT NULL,
    item_index INTEGER NOT NULL,
    added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT pk_gallery_video_gallery_video_index PRIMARY KEY (
        gallery_id, video_id, item_index
    ),
    CONSTRAINT fk_gallery_video_gallery FOREIGN KEY (
        gallery_id
    ) REFERENCES gallery (gallery_id),
    CONSTRAINT fk_gallery_video_video FOREIGN KEY (video_id) REFERENCES video (
        video_id
    )
);
