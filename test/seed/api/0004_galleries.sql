-- A public gallery owned by the demo account `alice` (user 9001) with one
-- image and one video, so both gallery media kinds resolve.
--
-- Image 9002 and video 9003 become USER through their gallery items.
--
-- A second, empty gallery gives the fixture API run a delete target that does
-- not disturb the gallery its reads address.

INSERT INTO gallery (
    gallery_id, name, created_at, last_modified_at, is_public, user_id
)
VALUES (9001, 'Seed Gallery', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 1, 9001);

INSERT INTO gallery (
    gallery_id, name, created_at, last_modified_at, is_public, user_id
)
VALUES (
    9002,
    'Seed Gallery Two',
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP,
    1,
    9001
);

INSERT INTO gallery_item (
    gallery_item_id, gallery_id, image_id, video_id, item_index
)
VALUES
(9001, 9001, 9002, NULL, 0),
(9002, 9001, NULL, 9003, 1);
