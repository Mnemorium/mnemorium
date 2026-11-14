import type { Flow } from "../config.ts";
import type { MnemoriumAPIClient } from "../generated/client.gen.ts";
import type { RunContext } from "../lib/auth.ts";

/** What a flow receives from the profile router. */
export type FlowContext = {
  /** Client carrying the token and this flow's tag. */
  client: MnemoriumAPIClient;
  /** The run context `setup()` produced. */
  ctx: RunContext;
  /** The flow's own name, used as its check and metric tag. */
  flow: Flow;
  /** One-based virtual user identifier. */
  vu: number;
  /** Zero-based iteration number for this VU. */
  iter: number;
};

/** One measured black-box flow. */
export type FlowFn = (context: FlowContext) => Promise<void>;
