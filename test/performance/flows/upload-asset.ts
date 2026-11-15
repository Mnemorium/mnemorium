// UC-006: create an upload session, send one chunk, replay it, and complete.

import { check } from "k6";
import crypto from "k6/crypto";

import { endpointTag } from "../config.ts";
import { assertion, atLeast, fields, idempotentReplay, isStatus } from "../lib/checks.ts";
import { trailingId } from "../lib/data.ts";

import type { FlowFn } from "./types.ts";

// `open` is init-context only and resolves against this file's directory.
const FIXTURE = new Uint8Array(open("../../seed/media/cover.png", "b"));
const COUNTER_BYTES = 8;

/** A fixture image made unique per iteration without changing its media type. */
function iterationBytes(vu: number, iter: number): ArrayBuffer {
  const bytes = new Uint8Array(FIXTURE.byteLength + COUNTER_BYTES);
  bytes.set(FIXTURE);
  const view = new DataView(bytes.buffer);
  view.setUint32(FIXTURE.byteLength, vu, true);
  view.setUint32(FIXTURE.byteLength + 4, iter, true);
  return bytes.buffer;
}

export const uploadAsset: FlowFn = async ({ client, flow, vu, iter }) => {
  const bytes = iterationBytes(vu, iter);
  const total = bytes.byteLength;
  const integrityHash = crypto.sha256(bytes, "hex");
  const digest = `sha-256=:${crypto.sha256(bytes, "base64")}:`;

  const session = await client.postUpload({
    body: {
      content_type: "image/png",
      file_name: `perf-${vu}-${iter}.png`,
      file_size: total,
      integrity_hash: integrityHash,
    },
    ...endpointTag("post-upload"),
  });
  if (!isStatus(session, flow, 201)) {
    return;
  }
  if (!fields(session.data, flow, "upload session", ["upload_id", "chunk_size", "expires_at"])) {
    return;
  }
  const uploadId = session.data.upload_id;
  assertion(flow, "upload declares a chunk", uploadId > 0 && session.data.chunk_size > 0);

  const chunkHeaders = {
    "Content-Digest": digest,
    "Content-Range": `bytes 0-${total - 1}/${total}`,
  };
  const chunk = await client.putUploadChunk(uploadId, 0, {
    body: bytes,
    headers: chunkHeaders,
    ...endpointTag("put-upload-chunk"),
  });
  if (!isStatus(chunk, flow, 200)) {
    return;
  }
  assertion(flow, "chunk received", chunk.data.bitmap.length === chunk.data.total_chunks);

  const replay = await client.putUploadChunk(uploadId, 0, {
    body: bytes,
    headers: chunkHeaders,
    ...endpointTag("put-upload-chunk"),
  });
  if (!isStatus(replay, flow, 200)) {
    return;
  }
  idempotentReplay(chunk.data.bitmap, replay.data.bitmap, flow, "chunk replay");

  const complete = await client.postUploadComplete(uploadId, endpointTag("post-upload-complete"));
  if (!isStatus(complete, flow, 200)) {
    return;
  }
  if (!fields(complete.data, flow, "completion", ["is_finished", "_links"])) {
    return;
  }
  assertion(flow, "upload finished", complete.data.is_finished);
  const fileId = trailingId(complete.data._links.file.href);
  atLeast(fileId, 1, flow, "completed file id");

  const second = await client.postUploadComplete(uploadId, endpointTag("post-upload-complete"));
  if (!isStatus(second, flow, 200)) {
    return;
  }
  assertion(flow, "completion replay returns the same file", trailingId(second.data._links.file.href) === fileId);

  const file = await client.getFile(fileId, endpointTag("get-uploaded-file"));
  if (!isStatus(file, flow, 200)) {
    return;
  }
  assertion(flow, "completed file is readable once", file.data.id === fileId);
  check(file.response, { [`${flow}: completed file size matches`]: () => file.data.size_bytes === total }, { flow });
};
