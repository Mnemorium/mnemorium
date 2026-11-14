// Two minutes at each flow's promoted target rate: the regression scenario.

import type { Scenario } from "k6/options";

import { perFlowScenarios, rateFor, STAGES, type Flow } from "../config.ts";

export function load(flows: readonly Flow[]): Record<string, Scenario> {
  return perFlowScenarios(flows, (flow) => {
    const rate = rateFor(flow);
    return {
      executor: "constant-arrival-rate",
      duration: STAGES.load.duration,
      maxVUs: Math.max(4, rate * 4),
      preAllocatedVUs: Math.max(2, Math.ceil(rate * 2)),
      rate,
      timeUnit: "1s",
    };
  });
}
