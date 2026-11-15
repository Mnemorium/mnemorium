// UC-022, UC-023: read a film and page a show's season twice.

import { DATASET, endpointTag } from "../config.ts";
import { assertion, atLeast, fields, idempotentReplay, isStatus, link } from "../lib/checks.ts";

import type { FlowFn } from "./types.ts";

export const browseVideo: FlowFn = async ({ client, flow }) => {
  const movies = await client.listMovies({
    query: { limit: DATASET.pageSize },
    ...endpointTag("list-movies"),
  });
  if (!isStatus(movies, flow, 200)) {
    return;
  }
  if (!fields(movies.data, flow, "movie page", ["_embedded", "_links"])) {
    return;
  }
  if (!fields(movies.data._embedded, flow, "movie page embedded", ["items"])) {
    return;
  }
  assertion(flow, "movie page has rows", movies.data._embedded.items.length > 0);

  const movie = movies.data._embedded.items[0];
  if (movie === undefined) {
    return;
  }
  assertion(flow, "movie in seeded range", movie.id >= DATASET.movieBase);
  if (!fields(movie, flow, "movie", ["id", "name", "_links"])) {
    return;
  }
  link(movie._links.self, flow, "movie self");

  const detail = await client.getMovie(movie.id, endpointTag("get-movie"));
  if (!isStatus(detail, flow, 200)) {
    return;
  }
  if (!fields(detail.data, flow, "movie detail", ["id", "name", "video", "_links"])) {
    return;
  }
  if (!fields(detail.data.video, flow, "movie video", ["file_id", "codec", "width", "height"])) {
    return;
  }
  atLeast(detail.data.video.file_id, DATASET.videoFileBase, flow, "movie video file_id");

  const shows = await client.listSeries({
    query: { limit: DATASET.pageSize },
    ...endpointTag("list-series"),
  });
  if (!isStatus(shows, flow, 200)) {
    return;
  }
  if (!fields(shows.data, flow, "series page", ["_embedded", "_links"])) {
    return;
  }
  if (!fields(shows.data._embedded, flow, "series page embedded", ["items"])) {
    return;
  }
  assertion(flow, "series page has rows", shows.data._embedded.items.length > 0);
  const show = shows.data._embedded.items[0];
  if (show === undefined) {
    return;
  }
  assertion(flow, "series in seeded range", show.id >= DATASET.seriesBase);

  const first = await client.listSeriesEpisodes(show.id, 1, endpointTag("list-series-episodes"));
  if (!isStatus(first, flow, 200)) {
    return;
  }
  if (!fields(first.data, flow, "episode page", ["items", "_links"])) {
    return;
  }
  assertion(flow, `episode count ${DATASET.episodesPerSeason}`, first.data.items.length === DATASET.episodesPerSeason);
  for (const episode of first.data.items) {
    assertion(flow, `episode ${episode.id} index in range`, episode.id >= 0 && episode.id < DATASET.episodesPerSeason);
    assertion(flow, `episode ${episode.id} season index`, episode.season_index === 1);
    atLeast(episode.video.file_id, DATASET.videoFileBase, flow, `episode ${episode.id} file_id`);
  }

  const second = await client.listSeriesEpisodes(show.id, 1, endpointTag("list-series-episodes"));
  if (!isStatus(second, flow, 200)) {
    return;
  }
  idempotentReplay(
    first.data.items.map((episode) => episode.id),
    second.data.items.map((episode) => episode.id),
    flow,
    "episode order",
  );
};
