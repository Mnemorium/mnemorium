// Authentication and the per-VU client factory.

import { ENV, type Flow } from "../config.ts";
import { MnemoriumAPIClient } from "../generated/client.gen.ts";

/** What `setup()` hands every VU. */
export type RunContext = {
  /** Bearer token acquired once in `setup()`. */
  token: string;
  /** API base, shared so a VU never reads the environment again. */
  baseUrl: string;
  /** Username the token belongs to. */
  username: string;
};

/** Build a client whose requests carry the token and the flow tag. */
export function clientFor(ctx: RunContext, flow: Flow): MnemoriumAPIClient {
  return tokenClient(ctx.baseUrl, ctx.token, flow);
}

/** Build a client for an explicit token and flow tag. */
export function tokenClient(baseUrl: string, token: string, flow: string): MnemoriumAPIClient {
  return new MnemoriumAPIClient({
    baseUrl,
    commonK6Params: {
      headers: { Authorization: `Bearer ${token}` },
      tags: { flow },
    },
  });
}

/** The base a scenario with no token uses, for `setup()` and the login flow. */
export function anonymousClient(): MnemoriumAPIClient {
  return new MnemoriumAPIClient({ baseUrl: ENV.baseUrl });
}
