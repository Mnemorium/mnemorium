// A fixed, low rate for a local `cargo flamegraph` capture: one profile at a
// time, so each capture names the work it measures.

import type { Scenario } from "k6/options";

import { STAGES } from "../config.ts";

export function flamegraph(): Record<string, Scenario> {
  const profile = __ENV.PERF_PROFILE ?? "all";
  const scenarios: Record<string, Scenario> = {};
  if (profile !== "write") {
    scenarios.read = {
      executor: "constant-arrival-rate",
      duration: STAGES.flamegraph.duration,
      exec: "readProfile",
      maxVUs: 16,
      preAllocatedVUs: 8,
      rate: STAGES.flamegraph.readRate,
      timeUnit: "1s",
    };
  }
  if (profile !== "read") {
    scenarios.write = {
      executor: "constant-arrival-rate",
      duration: STAGES.flamegraph.duration,
      exec: "writeProfile",
      maxVUs: 8,
      preAllocatedVUs: 4,
      rate: STAGES.flamegraph.writeRate,
      timeUnit: "1s",
    };
  }
  return scenarios;
}
