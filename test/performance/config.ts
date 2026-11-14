// The single place a scenario, a flow and a threshold are named.
//
// The dataset identifiers mirror `script/generate_perf_seed.sh` and the
// hardlink bases in `src/lib/infrastructure/seed.rs`. A drift shows up in the
// dataset gate before k6 starts, not as latencies measured on 404s.

import type { Options, Scenario as ScenarioDefinition } from "k6/options";

/** One of the environments a baseline may be recorded for. */
export type Environment = "ubuntu-latest" | "local";

/** A measured k6 scenario. */
export type Scenario = "smoke" | "load" | "stress" | "soak" | "flamegraph";

/** A traffic profile: read-only flows or write flows. */
export type Profile = "read" | "write";

/** One black-box flow the suite drives. */
export type Flow =
  | "login"
  | "browse-galleries"
  | "browse-music"
  | "browse-video"
  | "read-asset"
  | "upload-asset"
  | "create-gallery"
  | "curate-playlist";

/** The runtime environment, from `k6 --env` with development defaults. */
export const ENV = {
  baseUrl: __ENV.PERF_BASE_URL ?? "http://127.0.0.1:4080/api/v1",
  environment: (__ENV.PERF_ENVIRONMENT ?? "local") as Environment,
  password: __ENV.PERF_PASSWORD ?? "C0rrect!Horse",
  username: __ENV.PERF_USERNAME ?? "alice",
} as const;

/** The seeded volume library, in the identifiers the flows address. */
export const DATASET = {
  albumBase: 4_000_000,
  albums: 500,
  audioFileBase: 1_100_000,
  audioMediaBase: 2_100_000,
  episodesPerSeason: 10,
  galleryBase: 3_000_000,
  galleryItemBase: 3_100_000,
  galleryItemsPerGallery: 5,
  galleries: 1000,
  imageBytes: 1024,
  imageFileBase: 1_000_000,
  imageMediaBase: 2_000_000,
  mediumBase: 4_100_000,
  movieBase: 5_000_000,
  movies: 1000,
  musicGroupId: 4_900_000,
  pageSize: 20,
  playlistBase: 4_400_000,
  playlistTracks: 5,
  playlists: 100,
  recordingBase: 4_200_000,
  seasonBase: 6_100_000,
  seasonsPerSeries: 3,
  series: 200,
  seriesBase: 6_000_000,
  spareImages: 2048,
  streamBytes: 4_194_304,
  streamFileId: 1_200_000,
  tracksPerAlbum: 10,
  videoFileBase: 1_200_000,
  videoMediaBase: 2_200_000,
} as const;

/** The cheapest image file the read-asset flow reads whole. */
export const READ_IMAGE_FILE_ID = DATASET.imageFileBase;

/** The first unattached image file `create-gallery` attaches. */
export const SPARE_IMAGE_FILE_BASE = DATASET.imageFileBase + DATASET.galleries * DATASET.galleryItemsPerGallery;

/** The flow catalog: use cases and the profile that drives it. */
export const CATALOG: Record<Flow, { profile: Profile; useCases: readonly string[] }> = {
  login: { profile: "read", useCases: ["UC-002"] },
  "browse-galleries": { profile: "read", useCases: ["UC-008"] },
  "browse-music": { profile: "read", useCases: ["UC-012", "UC-014"] },
  "browse-video": { profile: "read", useCases: ["UC-022", "UC-023"] },
  "read-asset": { profile: "read", useCases: ["UC-031"] },
  "upload-asset": { profile: "write", useCases: ["UC-006"] },
  "create-gallery": { profile: "write", useCases: ["UC-007", "UC-009"] },
  "curate-playlist": { profile: "write", useCases: ["UC-013", "UC-016", "UC-018"] },
};

/** The read mix, in the order a stress or soak iteration rotates through it. */
export const READ_FLOWS: readonly Flow[] = ["login", "browse-galleries", "browse-music", "browse-video", "read-asset"];

/** The write mix. */
export const WRITE_FLOWS: readonly Flow[] = ["upload-asset", "create-gallery", "curate-playlist"];

/** Per-flow load rates used until a baseline replaces them. */
///
/// Provisional: these are the rates the harness offers before a baseline row
/// exists, not a capacity claim. The volume library makes the browse flows the
/// slowest — one VU measures 0.23–0.43 iterations/s on them — so driving them
/// at several iterations per second only exercises timeouts. A promoted
/// baseline replaces these numbers and becomes the rate a run is judged at.
const FALLBACK_RATE: Record<Flow, number> = {
  login: 2,
  "browse-galleries": 1,
  "browse-music": 1,
  "browse-video": 1,
  "read-asset": 2,
  "upload-asset": 1,
  "create-gallery": 1,
  "curate-playlist": 1,
};

/** The shape every scenario reads. */
export const STAGES = {
  smoke: { duration: "30s", vus: 1 },
  load: { duration: "2m" },
  stress: {
    stages: [
      { duration: "1m", target: 10 },
      { duration: "1m", target: 50 },
      { duration: "1m", target: 100 },
    ],
  },
  soak: { duration: "15m" },
  // A rate the SUT holds while perf samples it: the profiler, not the server,
  // is the bottleneck above this, and a starved arrival rate biases the stacks.
  flamegraph: { duration: "90s", readRate: 5, writeRate: 1 },
} as const;

/** A promoted baseline row. */
export type BaselineRow = {
  /** Completed flow iterations per second. */
  itersPerSecond: number;
  /** p99 of `http_req_duration`, in milliseconds. */
  p99Ms: number;
  /** p95 of `http_req_duration`, in milliseconds. */
  p95Ms: number;
};

/**
 * Promoted baselines, keyed per environment, scenario and flow.
 *
 * An empty section is the documented "no baseline yet" state: the report
 * still prints values, with `n/a` verdicts, and only the error-rate
 * thresholds are enforced.
 */
export const BASELINE: Record<Environment, Partial<Record<Scenario, Partial<Record<Flow, BaselineRow>>>>> = {
  local: {},
  "ubuntu-latest": {},
};

/** The threshold multipliers from the plan's threshold policy. */
export const POLICY = {
  itersFloor: 0.9,
  p95Ceiling: 1.2,
  p99Ceiling: 1.3,
  errorCeiling: 0.001,
} as const;

/** The flows the current run drives, from `PERF_FLOWS`/`PERF_PROFILE`. */
export function selectedFlows(): readonly Flow[] {
  const explicit = __ENV.PERF_FLOWS;
  if (explicit !== undefined && explicit.length > 0) {
    return explicit.split(",").filter((name): name is Flow => name in CATALOG);
  }
  const profile = __ENV.PERF_PROFILE ?? "all";
  if (profile === "read") {
    return selectedScenario() === "stress" ? [...READ_FLOWS, "upload-asset"] : READ_FLOWS;
  }
  if (profile === "write") {
    return WRITE_FLOWS;
  }
  return [...READ_FLOWS, ...WRITE_FLOWS];
}

/** Whether `flow` belongs to the profile the current scenario drives. */
export function isReadFlow(flow: Flow): boolean {
  return CATALOG[flow].profile === "read";
}

/** The declared scenario, defaulting to smoke. */
export function selectedScenario(): Scenario {
  const name = __ENV.SCENARIO ?? "smoke";
  return (["smoke", "load", "stress", "soak", "flamegraph"] as const).includes(name as Scenario)
    ? (name as Scenario)
    : "smoke";
}

/** The promoted baseline row for a flow, when the environment has one. */
export function baselineFor(flow: Flow): BaselineRow | undefined {
  return BASELINE[ENV.environment][selectedScenario()]?.[flow];
}

/** The load rate for a flow: its baseline, or the provisional fallback. */
export function rateFor(flow: Flow): number {
  return baselineFor(flow)?.itersPerSecond ?? FALLBACK_RATE[flow];
}

/**
 * Build the k6 threshold map for the selected flows.
 *
 * A flow with no promoted baseline still gets the error-rate thresholds:
 * a run that answers 500s must fail whether or not a latency budget exists.
 */
export function thresholdsFor(flows: readonly Flow[]): Options["thresholds"] {
  const thresholds: NonNullable<Options["thresholds"]> = {};
  const perFlowScenarios = selectedScenario() === "smoke" || selectedScenario() === "load";
  for (const flow of flows) {
    thresholds[`http_req_failed{flow:${flow}}`] = [`rate<${POLICY.errorCeiling}`];
    thresholds[`checks{flow:${flow}}`] = [`rate>${1 - POLICY.errorCeiling}`];
    // A threshold materialises the tagged submetric in `handleSummary`, so the
    // report can print it even before a baseline exists. The 60 s ceiling is a
    // hang guard, not a latency budget.
    thresholds[`http_req_duration{flow:${flow}}`] = ["p(95)<60000"];
    thresholds[`http_reqs{flow:${flow}}`] = ["rate>=0"];
    if (perFlowScenarios) {
      thresholds[`iterations{scenario:${flow}}`] = ["rate>=0"];
    }
    const baseline = baselineFor(flow);
    if (baseline !== undefined) {
      thresholds[`http_req_duration{flow:${flow}}`] = [
        `p(95)<${round(baseline.p95Ms * POLICY.p95Ceiling)}`,
        `p(99)<${round(baseline.p99Ms * POLICY.p99Ceiling)}`,
      ];
      if (perFlowScenarios) {
        thresholds[`iterations{scenario:${flow}}`] = [`rate>=${round(baseline.itersPerSecond * POLICY.itersFloor)}`];
      }
    }
  }
  return thresholds;
}

/** One scenario per selected flow, each routed through its profile function. */
export function perFlowScenarios(
  flows: readonly Flow[],
  scenario: (flow: Flow, profile: Profile, index: number) => ScenarioDefinition,
): Record<string, ScenarioDefinition> {
  const scenarios: Record<string, ScenarioDefinition> = {};
  flows.forEach((flow, index) => {
    const profile = CATALOG[flow].profile;
    scenarios[flow] = {
      ...scenario(flow, profile, index),
      env: { FLOW: flow },
      exec: profile === "read" ? "readProfile" : "writeProfile",
    };
  });
  return scenarios;
}

/** Round a threshold to a value k6 accepts. */
function round(value: number): number {
  return Math.round(value * 1000) / 1000;
}
