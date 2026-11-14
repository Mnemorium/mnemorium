// UC-002: authenticate and read the account back.

import { ACCOUNT } from "../data/accounts.ts";
import { assertion, fields, isStatus } from "../lib/checks.ts";
import { tokenClient } from "../lib/auth.ts";

import type { FlowFn } from "./types.ts";

export const login: FlowFn = async ({ client, ctx, flow }) => {
  const result = await client.postLogin({
    body: { password: ACCOUNT.password, username: ACCOUNT.username },
  });
  if (!isStatus(result, flow, 200)) {
    return;
  }

  const token = result.data.access_token;
  assertion(flow, "access_token present", token.length > 0);
  assertion(flow, "token_type is bearer", result.data.token_type === "bearer");

  const authenticated = tokenClient(ctx.baseUrl, token, flow);
  const me = await authenticated.getMe();
  if (!isStatus(me, flow, 200)) {
    return;
  }
  if (!fields(me.data, flow, "me", ["username", "role", "_links"])) {
    return;
  }
  assertion(flow, "me reports the authenticated username", me.data.username === ACCOUNT.username);
};
