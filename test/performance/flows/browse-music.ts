// UC-012, UC-014: page albums and playlists, then read an album's tracks twice.

import { DATASET } from "../config.ts";
import { assertion, atLeast, fields, idempotentReplay, isStatus, link } from "../lib/checks.ts";
import { pickOffset } from "../lib/data.ts";

import type { FlowFn } from "./types.ts";

// The "Unknown" album (id 0) is part of the collection, so the seeded range
// starts one row in.
const ALBUM_COLLECTION_TOTAL = DATASET.albums + 1;

export const browseMusic: FlowFn = async ({ client, flow, vu, iter }) => {
  const offset = pickOffset(ALBUM_COLLECTION_TOTAL, vu, iter);
  const page = await client.listAlbums({ query: { limit: DATASET.pageSize, offset } });
  if (!isStatus(page, flow, 200)) {
    return;
  }
  if (!fields(page.data, flow, "album page", ["_embedded", "_links"])) {
    return;
  }
  if (!fields(page.data._embedded, flow, "album page embedded", ["items"])) {
    return;
  }

  const albums = page.data._embedded.items;
  const expected = Math.max(0, Math.min(DATASET.pageSize, ALBUM_COLLECTION_TOTAL - offset));
  assertion(flow, `album page length ${expected}`, albums.length === expected);
  const album = albums[0];
  if (album === undefined) {
    assertion(flow, "first album present", false);
    return;
  }
  assertion(flow, "album in seeded range", album.id >= DATASET.albumBase);
  if (!fields(album, flow, "album", ["id", "name", "_links"])) {
    return;
  }
  link(album._links.self, flow, "album self");

  const first = await client.getAlbumTrackList(album.id, 1);
  if (!isStatus(first, flow, 200)) {
    return;
  }
  if (!fields(first.data, flow, "track list", ["items", "_links"])) {
    return;
  }
  assertion(flow, `album track count ${DATASET.tracksPerAlbum}`, first.data.items.length === DATASET.tracksPerAlbum);
  for (const track of first.data.items) {
    assertion(flow, `track ${track.id} index in range`, track.id >= 0 && track.id < DATASET.tracksPerAlbum);
    atLeast(track.file_id, DATASET.audioFileBase, flow, `track ${track.id} file_id`);
    assertion(flow, `track ${track.id} recording in range`, track.recording_id >= DATASET.recordingBase);
  }

  const second = await client.getAlbumTrackList(album.id, 1);
  if (!isStatus(second, flow, 200)) {
    return;
  }
  idempotentReplay(
    first.data.items.map((track) => track.id),
    second.data.items.map((track) => track.id),
    flow,
    "album track order",
  );

  const playlists = await client.listPlaylists({ query: { limit: 10 } });
  if (!isStatus(playlists, flow, 200)) {
    return;
  }
  if (!fields(playlists.data, flow, "playlist page", ["items", "_links"])) {
    return;
  }
  const playlist = playlists.data.items.find((item) => item.id >= DATASET.playlistBase);
  if (playlist === undefined) {
    assertion(flow, "seeded playlist present", false);
    return;
  }
  if (!fields(playlist, flow, "playlist", ["id", "name", "track_count", "_links"])) {
    return;
  }
  const detail = await client.getPlaylist(playlist.id);
  if (!isStatus(detail, flow, 200)) {
    return;
  }
  if (!fields(detail.data, flow, "playlist detail", ["track_count", "tracks"])) {
    return;
  }
  assertion(flow, `playlist track count ${DATASET.playlistTracks}`, detail.data.track_count === DATASET.playlistTracks);
  assertion(flow, "playlist track count matches", detail.data.track_count === detail.data.tracks.length);
};
