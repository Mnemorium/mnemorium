// UC-013, UC-016, UC-018: create a playlist, append a recording, reorder it,
// and confirm the representation reports exactly one track.

import { endpointTag } from "../config.ts";
import { assertion, fields, isStatus, sideEffect } from "../lib/checks.ts";
import { recordingId, uniqueName } from "../lib/data.ts";

import type { FlowFn } from "./types.ts";

export const curatePlaylist: FlowFn = async ({ client, flow, vu, iter }) => {
  const created = await client.postPlaylist({
    body: { is_public: true, name: uniqueName("perf-playlist", vu, iter) },
    ...endpointTag("post-playlist"),
  });
  if (!isStatus(created, flow, 201)) {
    return;
  }
  if (!fields(created.data, flow, "created playlist", ["id", "name", "track_count", "_links"])) {
    return;
  }
  assertion(flow, "a new playlist is empty", created.data.track_count === 0);
  const playlistId = created.data.id;
  const recording = recordingId(vu, iter);

  const appended = await client.patchPlaylist(playlistId, {
    body: { add_track: { recording_id: recording } },
    ...endpointTag("patch-playlist"),
  });
  if (!isStatus(appended, flow, 200)) {
    return;
  }
  sideEffect(appended.data, flow, "track_count", 1);
  assertion(flow, "appended track has index 0", appended.data.tracks[0]?.id === 0);
  assertion(flow, "appended track places the recording", appended.data.tracks[0]?.recording_id === recording);

  const reordered = await client.putPlaylistTrack(playlistId, {
    body: { order: [0] },
    ...endpointTag("put-playlist-track"),
  });
  if (!isStatus(reordered, flow, 200)) {
    return;
  }
  if (!fields(reordered.data, flow, "reordered playlist", ["track_count", "tracks"])) {
    return;
  }
  assertion(flow, "reorder is a permutation", reordered.data.track_count === 1);
  assertion(flow, "reorder keeps index 0", reordered.data.tracks[0]?.id === 0);

  const after = await client.getPlaylist(playlistId, endpointTag("get-curated-playlist"));
  if (!isStatus(after, flow, 200)) {
    return;
  }
  if (!fields(after.data, flow, "playlist detail", ["track_count", "tracks"])) {
    return;
  }
  assertion(flow, "no duplicate track row", after.data.track_count === 1);
  assertion(flow, "track indices are unique", new Set(after.data.tracks.map((track) => track.id)).size === 1);
  // No destructive teardown: see `docs/development/Test.md`, "Known
  // limitations". The container's authorization resource row outlives a
  // delete, and the reused identifier rejects the next create.
};
