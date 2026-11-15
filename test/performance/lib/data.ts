// Deterministic selection over the seeded library.
//
// Nothing here is random: the same VU and iteration always address the same
// rows, so a failure names a reproducible identifier and two reads in one
// iteration can be compared.

import { DATASET, READ_IMAGE_FILE_ID, SPARE_IMAGE_FILE_BASE, type Flow } from "../config.ts";

/** The gallery collection includes the instance's default gallery id 0. */
export const GALLERY_TOTAL = DATASET.galleries + 1;

/** A reproducible index in `[0, count)`. */
export function pickIndex(count: number, vu: number, iter: number): number {
  if (count <= 0) {
    return 0;
  }
  return ((vu - 1) * 7919 + iter * 104_729) % count;
}

/** A reproducible offset that keeps a whole page inside the collection. */
export function pickOffset(count: number, vu: number, iter: number): number {
  const highest = Math.max(1, count - DATASET.pageSize);
  return 1 + (pickIndex(highest, vu, iter) % highest);
}

/** The page length a collection of `count` rows returns at `offset`. */
export function pageLengthFor(count: number, offset: number): number {
  return Math.max(0, Math.min(DATASET.pageSize, count - offset));
}

/** An unattached image file the create-gallery flow attaches, unique per VU. */
export function spareImageFileId(vu: number, iter: number): number {
  const slot = ((vu - 1) * 100_000 + iter) % DATASET.spareImages;
  return SPARE_IMAGE_FILE_BASE + slot;
}

/** A seeded music recording a playlist can append. */
export function recordingId(vu: number, iter: number): number {
  return DATASET.recordingBase + pickIndex(DATASET.albums * DATASET.tracksPerAlbum, vu, iter);
}

/** A per-iteration unique name, so a count check cannot pass on a stale row. */
export function uniqueName(prefix: string, vu: number, iter: number): string {
  return `${prefix}-${vu}-${iter}`;
}

/** The image file the read-asset flow reads whole. */
export function imageFileId(): number {
  return READ_IMAGE_FILE_ID;
}

/** Parse the trailing identifier of a HAL link such as `/api/v1/asset/file/42`. */
export function trailingId(href: string): number {
  const match = /\/(\d+)(?:\?.*)?$/.exec(href);
  if (match === null) {
    return Number.NaN;
  }
  return Number.parseInt(match[1] ?? "", 10);
}

/** The flows a profile runs when no single flow was selected. */
export function flowForIteration(flows: readonly Flow[], iter: number): Flow {
  const flow = flows[iter % flows.length];
  if (flow === undefined) {
    throw new Error("the profile has no flows");
  }
  return flow;
}
