// The seeded demo identities. The password is compiled test data, printed by
// every `test-seed` boot; it is not an operational secret.

import { ENV } from "../config.ts";

/** The account every measured flow authenticates as. */
export const ACCOUNT = {
  username: ENV.username,
  password: ENV.password,
} as const;
