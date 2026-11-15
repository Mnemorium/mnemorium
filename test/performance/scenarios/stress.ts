// A VU ladder to 100 with one upload worker: finds the knee, not a baseline.

import type { Scenario } from "k6/options";

import { STAGES } from "../config.ts";

export function stress(): Record<string, Scenario> {
  return {
    read: {
      executor: "ramping-vus",
      exec: "readProfile",
      stages: [...STAGES.stress.stages],
      startVUs: 0,
    },
    "upload-asset": {
      executor: "constant-vus",
      duration: "3m",
      env: { FLOW: "upload-asset" },
      exec: "writeProfile",
      vus: 1,
    },
  };
}
