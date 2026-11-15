// UC-007, UC-009: create a gallery, attach one item, verify the row count, and
// tear the gallery down.

import { assertion, countDelta, fields, isStatus } from "../lib/checks.ts";
import { spareImageFileId, uniqueName } from "../lib/data.ts";

import type { FlowFn } from "./types.ts";

export const createGallery: FlowFn = async ({ client, flow, vu, iter }) => {
  const name = uniqueName("perf-gallery", vu, iter);
  const before = await client.listGalleries({ query: { limit: 10, name } });
  if (!isStatus(before, flow, 200)) {
    return;
  }
  assertion(flow, "created name is unique", before.data.items.length === 0);

  const created = await client.postGallery({ body: { is_public: true, name } });
  if (!isStatus(created, flow, 201)) {
    return;
  }
  if (!fields(created.data, flow, "created gallery", ["id", "name", "_links"])) {
    return;
  }
  assertion(flow, "created gallery has the requested name", created.data.name === name);
  assertion(
    flow,
    "created gallery reports its location",
    created.response.headers["Location"] === `/api/v1/library/gallery/${created.data.id}`,
  );

  const after = await client.listGalleries({ query: { limit: 10, name } });
  if (!isStatus(after, flow, 200)) {
    return;
  }
  countDelta(before.data.items.length, after.data.items.length, flow, "gallery");

  const fileId = spareImageFileId(vu, iter);
  const item = await client.postGalleryItem(created.data.id, {
    body: { media_id: fileId, type: "image" },
  });
  if (!isStatus(item, flow, 201)) {
    return;
  }
  assertion(flow, "the attached item is the seeded file", item.data.file_id === fileId);

  const detail = await client.getGallery(created.data.id);
  if (!isStatus(detail, flow, 200)) {
    return;
  }
  assertion(flow, "the gallery holds exactly one item", detail.data.item_count === 1);
  assertion(flow, "the item is reported once", detail.data.items.length === 1);
  // No destructive teardown: see `docs/development/Test.md`, "Known
  // limitations". Deleting an entity leaves its authorization resource row
  // behind, and SQLite then reuses the deleted identifier, so the next
  // iteration's create would answer 500. The system suite owns that defect.
};
