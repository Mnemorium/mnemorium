// The single k6 entry point.
//
// `SCENARIO` selects the shape and `PERF_FLOWS`/`PERF_PROFILE` the flows. The
// scenario objects route every flow through one of two named functions,
// because k6 allows one default export per run.

import { fail } from "k6";
import type { Options } from "k6/options";

import {
  CATALOG,
  DATASET,
  ENV,
  READ_FLOWS,
  selectedFlows,
  selectedScenario,
  thresholdsFor,
  WRITE_FLOWS,
  type Flow,
} from "./config.ts";
import { ACCOUNT } from "./data/accounts.ts";
import { FLOWS } from "./flows/index.ts";
import { anonymousClient, clientFor, tokenClient, type RunContext } from "./lib/auth.ts";
import { flowForIteration } from "./lib/data.ts";
import { renderSummary, type SummaryData } from "./lib/report.ts";
import { build } from "./scenarios/index.ts";

const flows = selectedFlows();
const scenario = selectedScenario();

/** The program options for the selected scenario. */
export const options: Options = {
  discardResponseBodies: false,
  scenarios: build(scenario, flows),
  summaryTrendStats: ["avg", "min", "med", "p(90)", "p(95)", "p(99)", "max"],
  thresholds: {
    ...thresholdsFor(flows),
    ...(scenario === "stress"
      ? {
          http_req_failed: [{ abortOnFail: true, delayAbortEval: "10s", threshold: `rate<${0.01}` }],
        }
      : {}),
  },
};

/** Authenticate once and refuse to run against an ungrounded library. */
export async function setup(): Promise<RunContext> {
  const client = anonymousClient();
  const login = await client.postLogin({
    body: { password: ACCOUNT.password, username: ACCOUNT.username },
  });
  if (login.status !== 200) {
    fail(`setup: login answered ${login.status}`);
  }

  const ctx: RunContext = {
    baseUrl: ENV.baseUrl,
    token: login.data.access_token,
    username: ACCOUNT.username,
  };
  await datasetGate(ctx);
  return ctx;
}

/** Stop the run when the library is not the seeded volume library. */
async function datasetGate(ctx: RunContext): Promise<void> {
  const client = tokenClient(ctx.baseUrl, ctx.token, "setup");

  const galleries = await client.listGalleries({ query: { limit: 100, offset: 1 } });
  if (galleries.status !== 200 || galleries.data.items.length !== 100) {
    fail("dataset not grounded: the first gallery page is not full");
  }
  const firstGallery = galleries.data.items[0];
  if (firstGallery === undefined || firstGallery.id < DATASET.galleryBase || firstGallery.item_count === 0) {
    fail("dataset not grounded: the gallery range is absent");
  }

  const albums = await client.listAlbums({ query: { limit: 1, offset: 1 } });
  const firstAlbum = albums.status === 200 ? albums.data._embedded.items[0] : undefined;
  if (firstAlbum === undefined || firstAlbum.id < DATASET.albumBase) {
    fail("dataset not grounded: the album range is absent");
  }

  const movies = await client.listMovies({ query: { limit: 1 } });
  const firstMovie = movies.status === 200 ? movies.data._embedded.items[0] : undefined;
  if (firstMovie === undefined || firstMovie.id < DATASET.movieBase) {
    fail("dataset not grounded: the movie range is absent");
  }

  const shows = await client.listSeries({ query: { limit: 1 } });
  const firstShow = shows.status === 200 ? shows.data._embedded.items[0] : undefined;
  if (firstShow === undefined || firstShow.id < DATASET.seriesBase) {
    fail("dataset not grounded: the series range is absent");
  }

  const episodes = await client.listSeriesEpisodes(firstShow.id, 1);
  if (episodes.status !== 200 || episodes.data.items.length !== DATASET.episodesPerSeason) {
    fail("dataset not grounded: the episode range is absent");
  }

  const stream = await client.getFile(DATASET.streamFileId);
  if (stream.status !== 200 || stream.data.size_bytes !== DATASET.streamBytes) {
    fail("dataset not grounded: the stream asset is absent");
  }
}

/** The flow an explicit `FLOW` names, or the mix's next flow. */
function chooseFlow(mix: readonly Flow[]): Flow {
  const explicit = __ENV.FLOW;
  if (explicit !== undefined && mix.includes(explicit as Flow)) {
    return explicit as Flow;
  }
  return flowForIteration(mix, __ITER);
}

/** Run one read flow for this iteration. */
export async function readProfile(ctx: RunContext): Promise<void> {
  const flow = chooseFlow(READ_FLOWS);
  const client = clientFor(ctx, flow);
  await FLOWS[flow]({ client, ctx, flow, iter: __ITER, vu: __VU });
}

/** Run one write flow for this iteration. */
export async function writeProfile(ctx: RunContext): Promise<void> {
  const flow = chooseFlow(WRITE_FLOWS);
  const client = clientFor(ctx, flow);
  await FLOWS[flow]({ client, ctx, flow, iter: __ITER, vu: __VU });
}

/** Write the Markdown report and the raw summary under the run directory. */
export function handleSummary(data: SummaryData): Record<string, string> {
  return renderSummary(data);
}

// Keeps the profile split visible to a reader who edits the catalog.
for (const flow of Object.keys(CATALOG) as Flow[]) {
  if (CATALOG[flow].profile === "read" && !READ_FLOWS.includes(flow)) {
    throw new Error(`${flow} is a read flow but is missing from READ_FLOWS`);
  }
}
