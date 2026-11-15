// One VU per selected flow for 30 s, flows one after another: the scheduled CI
// scenario. Staggering keeps a slow read flow from contaminating another
// flow's latency on the two-CPU SUT.

import type { Scenario } from "k6/options";

import { perFlowScenarios, STAGES, type Flow } from "../config.ts";

const GAP_SECONDS = 35;

export function smoke(flows: readonly Flow[]): Record<string, Scenario> {
  return perFlowScenarios(flows, (_flow, _profile, index) => ({
    executor: "constant-vus",
    duration: STAGES.smoke.duration,
    startTime: `${index * GAP_SECONDS}s`,
    vus: STAGES.smoke.vus,
  }));
}
