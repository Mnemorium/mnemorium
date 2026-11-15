// UC-031: stream stored bytes, whole and as a bounded range.

import { DATASET, endpointTag, READ_IMAGE_FILE_ID } from "../config.ts";
import { assertion, fields, isStatus } from "../lib/checks.ts";

import type { FlowFn } from "./types.ts";

/** The k6 response body length, whether it was decoded as text or binary. */
function byteLength(body: unknown): number {
  if (typeof body === "string") {
    return body.length;
  }
  if (body instanceof ArrayBuffer) {
    return body.byteLength;
  }
  return -1;
}

export const readAsset: FlowFn = async ({ client, flow }) => {
  const image = await client.getImageContent(
    DATASET.imageMediaBase,
    endpointTag("get-image-content", { responseType: "binary" }),
  );
  if (!isStatus(image, flow, 200)) {
    return;
  }
  assertion(flow, `image content length ${DATASET.imageBytes}`, byteLength(image.data) === DATASET.imageBytes);

  const whole = await client.getFileContent(
    READ_IMAGE_FILE_ID,
    endpointTag("get-file-content", { responseType: "binary" }),
  );
  if (!isStatus(whole, flow, 200)) {
    return;
  }
  assertion(flow, "file content length matches its header", byteLength(whole.data) === DATASET.imageBytes);
  assertion(
    flow,
    "file content header matches the seeded size",
    whole.response.headers["Content-Length"] === String(DATASET.imageBytes),
  );

  const rangeStart = 0;
  const rangeEnd = 1023;
  const ranged = await client.getFileContent(DATASET.streamFileId, {
    headers: { Range: `bytes=${rangeStart}-${rangeEnd}` },
    ...endpointTag("get-file-content", { responseType: "binary" }),
  });
  if (!isStatus(ranged, flow, 206)) {
    return;
  }
  assertion(flow, "range length matches its window", byteLength(ranged.data) === rangeEnd - rangeStart + 1);
  assertion(
    flow,
    "content-range names the seeded stream",
    ranged.response.headers["Content-Range"] === `bytes ${rangeStart}-${rangeEnd}/${DATASET.streamBytes}`,
  );

  const metadata = await client.getFile(DATASET.streamFileId, endpointTag("get-file"));
  if (!isStatus(metadata, flow, 200)) {
    return;
  }
  if (!fields(metadata.data, flow, "stream file", ["id", "size_bytes", "mime_type", "path"])) {
    return;
  }
  assertion(flow, "stream file size matches the base", metadata.data.size_bytes === DATASET.streamBytes);
  assertion(flow, "stream file is a video", metadata.data.mime_type === "video/mp4");
};
