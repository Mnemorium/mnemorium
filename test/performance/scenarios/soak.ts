// Fifteen minutes of the read mix at the target rate: manual only.

import type { Scenario } from "k6/options";

import { rateFor, READ_FLOWS, STAGES } from "../config.ts";

export function soak(): Record<string, Scenario> {
  const rate = READ_FLOWS.reduce((total, flow) => total + rateFor(flow), 0);
  return {
    read: {
      executor: "constant-arrival-rate",
      duration: STAGES.soak.duration,
      exec: "readProfile",
      maxVUs: Math.max(8, rate * 4),
      preAllocatedVUs: Math.max(4, Math.ceil(rate * 2)),
      rate,
      timeUnit: "1s",
    },
  };
}
