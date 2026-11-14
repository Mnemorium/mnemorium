// UC-008: page the gallery collection, then read one gallery and its items.

import { DATASET } from "../config.ts";
import { assertion, atLeast, fields, isStatus, link, present } from "../lib/checks.ts";
import { GALLERY_TOTAL, pickOffset } from "../lib/data.ts";

import type { FlowFn } from "./types.ts";

export const browseGalleries: FlowFn = async ({ client, flow, vu, iter }) => {
  const offset = pickOffset(GALLERY_TOTAL, vu, iter);
  const page = await client.listGalleries({
    query: { limit: DATASET.pageSize, offset },
  });
  if (!isStatus(page, flow, 200)) {
    return;
  }
  if (!fields(page.data, flow, "page", ["items", "_links"])) {
    return;
  }
  if (!fields(page.data._links, flow, "page links", ["self", "search"])) {
    return;
  }

  const rows = page.data.items;
  const expected = Math.max(0, Math.min(DATASET.pageSize, GALLERY_TOTAL - offset));
  const hasNext = offset + rows.length < GALLERY_TOTAL;
  assertion(flow, `page length ${expected}`, rows.length === expected);
  assertion(flow, "next link presence", present(page.data._links.next) === hasNext);
  assertion(flow, "prev link present", present(page.data._links.prev));

  const first = rows[0];
  if (first === undefined) {
    assertion(flow, "first gallery present", false);
    return;
  }

  for (const gallery of rows) {
    assertion(flow, `gallery ${gallery.id} in seeded range`, gallery.id >= DATASET.galleryBase);
    atLeast(gallery.item_count, 1, flow, `gallery ${gallery.id} item_count`);
    if (!fields(gallery, flow, `gallery ${gallery.id}`, ["id", "name", "_links"])) {
      return;
    }
    if (!link(gallery._links.self, flow, `gallery ${gallery.id} self`)) {
      return;
    }
  }

  const detail = await client.getGallery(first.id);
  if (!isStatus(detail, flow, 200)) {
    return;
  }
  if (!fields(detail.data, flow, "gallery", ["id", "items", "_links"])) {
    return;
  }
  assertion(flow, "item_count matches the detail", detail.data.item_count === detail.data.items.length);
  assertion(flow, "gallery has gallery items", detail.data.items.length > 0);

  for (const item of detail.data.items) {
    if (!fields(item, flow, `item ${item.id}`, ["id", "file_id", "media_id", "_links", "type", "name"])) {
      return;
    }
    atLeast(item.file_id, 1, flow, `item ${item.id} file_id`);
    atLeast(item.media_id, DATASET.imageMediaBase, flow, `item ${item.id} media_id`);
    atLeast(item.id, DATASET.galleryItemBase, flow, `item ${item.id} id`);
    link(item._links.self, flow, `item ${item.id} self`);
  }

  // The item route is a documented literal alias: the representation must be
  // byte-identical, not merely equivalent.
  const alias = await client.getGalleryItemList(first.id);
  if (!isStatus(alias, flow, 200)) {
    return;
  }
  assertion(flow, "item alias is body-identical", detail.response.body === alias.response.body);
};
