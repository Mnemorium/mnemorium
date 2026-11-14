import type { Scenario as ScenarioDefinition } from "k6/options";

import type { Flow, Scenario } from "../config.ts";

import { flamegraph } from "./flamegraph.ts";
import { load } from "./load.ts";
import { smoke } from "./smoke.ts";
import { soak } from "./soak.ts";
import { stress } from "./stress.ts";

/** Build the scenario map for the selected scenario and flows. */
export function build(scenario: Scenario, flows: readonly Flow[]): Record<string, ScenarioDefinition> {
  switch (scenario) {
    case "smoke":
      return smoke(flows);
    case "load":
      return load(flows);
    case "stress":
      return stress();
    case "soak":
      return soak();
    case "flamegraph":
      return flamegraph();
  }
}
