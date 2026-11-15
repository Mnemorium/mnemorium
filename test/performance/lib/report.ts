// The Markdown report and the promoted-baseline verdicts.
//
// k6 owns the metrics; this module only formats them. A flow with a promoted
// baseline gets PASS/FAIL, and a flow without one is reported with `n/a`, as
// the threshold policy requires.

import {
  BASELINE,
  ENV,
  ENDPOINT_CATALOG,
  POLICY,
  selectedFlows,
  selectedScenario,
  type Endpoint,
  type Flow,
  type Scenario,
} from "../config.ts";

/** One k6 metric in the summary tree. */
type SummaryMetric = {
  values: Record<string, number>;
  thresholds?: Record<string, { ok: boolean }>;
};

/** The part of `handleSummary`'s argument this module reads. */
export type SummaryData = {
  metrics: Record<string, SummaryMetric>;
};

/** The optional fingerprint the shell computes before the run. */
export type Fingerprint = Record<string, string | number>;

/** One rendered row. */
type Row = {
  flow: Flow;
  iterationsPerSecond: number | null;
  requestsPerSecond: number | null;
  p50Ms: number | null;
  p95Ms: number | null;
  p99Ms: number | null;
  httpErrorRate: number | null;
  checkErrorRate: number | null;
  verdict: string;
};

/** Read a fingerprint from the run environment. */
export function fingerprint(): Fingerprint {
  const raw = __ENV.PERF_FINGERPRINT;
  if (raw === undefined || raw.length === 0) {
    return { commit: __ENV.PERF_COMMIT ?? "unknown", environment: ENV.environment };
  }
  try {
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed === "object" && parsed !== null) {
      return parsed as Fingerprint;
    }
  } catch {
    // A malformed fingerprint is reported as absent, never as a run failure:
    // it is provenance, not a measurement.
  }
  return { environment: ENV.environment };
}

/**
 * The counter rate.
 *
 * A smoke scenario per flow starts after the previous one ends, so k6's
 * whole-run rate dilutes each counter by the stagger. The scenario's own
 * thirty-second window is the denominator the plan reports.
 */
function rate(data: SummaryData, name: string): number | null {
  const metric = data.metrics[name];
  if (metric === undefined) {
    return null;
  }
  if (selectedScenario() === "smoke" && typeof metric.values.count === "number") {
    return metric.values.count / 30;
  }
  return typeof metric.values.rate === "number" ? metric.values.rate : null;
}

/** One value of a trend metric. */
function trend(data: SummaryData, name: string, key: string): number | null {
  const value = data.metrics[name]?.values[key];
  return typeof value === "number" ? value : null;
}

/** Build one row from the tagged submetrics. */
function rowFor(data: SummaryData, flow: Flow): Row {
  const p50Ms = trend(data, `http_req_duration{flow:${flow}}`, "med");
  const p95Ms = trend(data, `http_req_duration{flow:${flow}}`, "p(95)");
  const p99Ms = trend(data, `http_req_duration{flow:${flow}}`, "p(99)");
  const httpErrorRate = rate(data, `http_req_failed{flow:${flow}}`);
  const checkRate = rate(data, `checks{flow:${flow}}`);
  const checkErrorRate = checkRate === null ? null : 1 - checkRate;
  const iterationsPerSecond = rate(data, `iterations{scenario:${flow}}`);
  const requestsPerSecond = rate(data, `http_reqs{flow:${flow}}`);

  return {
    checkErrorRate,
    flow,
    httpErrorRate,
    iterationsPerSecond,
    p50Ms,
    p95Ms,
    p99Ms,
    requestsPerSecond,
    verdict: verdictFor(flow, {
      checkErrorRate,
      httpErrorRate,
      iterationsPerSecond,
      p95Ms,
      p99Ms,
    }),
  };
}

/**
 * One row of the per-endpoint table.
 *
 * Diagnosis only: the reader uses it to name the slow call, and no verdict
 * reads it. A request whose flow forgot its tag leaves the row's values absent,
 * which prints as an em dash rather than dropping the row.
 */
function endpointLine(data: SummaryData, flow: Flow, endpoint: Endpoint): string {
  const duration = `http_req_duration{endpoint:${endpoint}}`;
  const requestsPerSecond = rate(data, `http_reqs{endpoint:${endpoint}}`);
  return (
    `| ${flow} | ${endpoint} | ${number(requestsPerSecond, 2)} | ` +
    `${number(trend(data, duration, "med"), 2)} | ${number(trend(data, duration, "p(95)"), 2)} | ` +
    `${number(trend(data, duration, "p(99)"), 2)} |`
  );
}

/** The verdict for one flow under the promoted-baseline policy. */
function verdictFor(
  flow: Flow,
  measured: {
    checkErrorRate: number | null;
    httpErrorRate: number | null;
    iterationsPerSecond: number | null;
    p95Ms: number | null;
    p99Ms: number | null;
  },
): string {
  const baseline = BASELINE[ENV.environment][selectedScenario()]?.[flow];
  if (baseline === undefined) {
    return "n/a";
  }
  if (measured.httpErrorRate === null || measured.checkErrorRate === null) {
    return "n/a";
  }
  if (measured.httpErrorRate > POLICY.errorCeiling || measured.checkErrorRate > POLICY.errorCeiling) {
    return "FAIL (errors)";
  }
  if (measured.p95Ms === null || measured.p99Ms === null) {
    return "n/a";
  }
  if (measured.p95Ms > baseline.p95Ms * POLICY.p95Ceiling) {
    return "FAIL (p95)";
  }
  if (measured.p99Ms > baseline.p99Ms * POLICY.p99Ceiling) {
    return "FAIL (p99)";
  }
  if (
    measured.iterationsPerSecond !== null &&
    measured.iterationsPerSecond < baseline.itersPerSecond * POLICY.itersFloor
  ) {
    return "FAIL (throughput)";
  }
  return "PASS";
}

/** Format a number with a fixed precision, or an em dash for absent values. */
function number(value: number | null, precision: number): string {
  return value === null ? "—" : value.toFixed(precision);
}

/** Format a fraction as a percentage. */
function percent(value: number | null): string {
  return value === null ? "—" : `${(value * 100).toFixed(3)}%`;
}

/** Render the whole report. */
export function render(data: SummaryData, scenario?: Scenario): string {
  const activeScenario = scenario ?? selectedScenario();
  const rows = selectedFlows().map((flow) => rowFor(data, flow));
  const environment = ENV.environment;
  const provenance = fingerprint();
  const promoted = BASELINE[environment][activeScenario] ?? {};

  const lines: string[] = [
    `# Performance report — ${activeScenario} (${environment})`,
    "",
    "## Run",
    "",
    `- Scenario: \`${activeScenario}\``,
    `- Environment: \`${environment}\``,
    `- Base URL: \`${ENV.baseUrl}\``,
    `- Promoted baseline rows: ${Object.keys(promoted).length}`,
    "",
    "| key | value |",
    "| --- | --- |",
  ];
  for (const [key, value] of Object.entries(provenance)) {
    lines.push(`| ${key} | ${String(value)} |`);
  }

  lines.push(
    "",
    "## Flows",
    "",
    "| flow | iters/s | req/s | p50 ms | p95 ms | p99 ms | http err | check err | verdict |",
    "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |",
  );
  for (const row of rows) {
    lines.push(
      `| ${row.flow} | ${number(row.iterationsPerSecond, 2)} | ${number(row.requestsPerSecond, 2)} | ` +
        `${number(row.p50Ms, 2)} | ${number(row.p95Ms, 2)} | ${number(row.p99Ms, 2)} | ` +
        `${percent(row.httpErrorRate)} | ${percent(row.checkErrorRate)} | ${row.verdict} |`,
    );
  }

  lines.push(
    "",
    "## Endpoints",
    "",
    "| flow | endpoint | req/s | p50 ms | p95 ms | p99 ms |",
    "| --- | --- | ---: | ---: | ---: | ---: |",
  );
  for (const flow of selectedFlows()) {
    for (const endpoint of ENDPOINT_CATALOG[flow]) {
      lines.push(endpointLine(data, flow, endpoint));
    }
  }

  lines.push(
    "",
    "## Threshold policy",
    "",
    `- p95 at most ${(POLICY.p95Ceiling * 100 - 100).toFixed(0)}% above its baseline.`,
    `- p99 at most ${(POLICY.p99Ceiling * 100 - 100).toFixed(0)}% above its baseline.`,
    `- Throughput at least ${(POLICY.itersFloor * 100).toFixed(0)}% of its baseline.`,
    `- HTTP and business-check failure rates below ${(POLICY.errorCeiling * 100).toFixed(1)}%.`,
    "",
    "A flow with no promoted row reports `n/a`: its values are recorded, but they",
    "do not gate the run. `Baseline.md` carries the promoted numbers and the",
    "fingerprint they were measured on.",
    "",
    "## Non-goals",
    "",
    "- Response-schema validation: owned by the system suite and Schemathesis.",
    "- Event or message publication: the server has no broker.",
    "- Configuration, administration and provider-backed metadata flows.",
    "",
  );
  return lines.join("\n");
}

/** Render and serialise the summary k6 leaves behind. */
export function renderSummary(data: SummaryData): Record<string, string> {
  const scenario = selectedScenario();
  const directory = __ENV.PERF_REPORT_DIR ?? ".artifacts/performance/latest";
  const markdown = render(data, scenario);
  return {
    [`${directory}/report.md`]: markdown,
    [`${directory}/summary.json`]: `${JSON.stringify(data, null, 2)}\n`,
    stdout: `\n${markdown}\n`,
  };
}
