// The business checks every flow records.
//
// Each helper emits one tagged `check()`, so `checks{flow:<name>}` is the
// per-flow business-failure rate and a failed assertion names its flow.

import { check } from "k6";
import type { Response } from "k6/http";

/** The part of a generated-client result the status check reads. */
export type StatusResult = {
  response: Response;
  status: number;
};

/** Record one tagged assertion. */
export function assertion(flow: string, label: string, condition: boolean): void {
  check({}, { [`${flow}: ${label}`]: () => condition }, { flow });
}

/** Assert the HTTP status is one of `expected`. */
export function status(result: StatusResult, flow: string, expected: number | readonly number[]): boolean {
  const accepted = typeof expected === "number" ? [expected] : expected;
  const ok = accepted.includes(result.status);
  assertion(flow, `status ${accepted.join("|")}`, ok);
  return ok;
}

/**
 * Assert the status and narrow the result's `data` to that status.
 *
 * The generated client models every status as a union member carrying its own
 * body type, so a plain boolean check cannot make `data` safe to read.
 */
export function isStatus<S extends number, R extends StatusResult>(
  result: R,
  flow: string,
  expected: S,
): result is Extract<R, { status: S }> {
  return status(result, flow, expected);
}

/** Assert `value` is a non-null object. */
export function object(value: unknown, flow: string, label: string): value is Record<string, unknown> {
  const ok = typeof value === "object" && value !== null && !Array.isArray(value);
  assertion(flow, `${label} is an object`, ok);
  return ok;
}

/** Whether a value is present and not an empty string or empty array. */
export function present(value: unknown): boolean {
  if (value === undefined || value === null) {
    return false;
  }
  if (typeof value === "string") {
    return value.length > 0;
  }
  if (Array.isArray(value)) {
    return value.length > 0;
  }
  return true;
}

/** Assert every named field is present and non-empty. */
export function fields(value: unknown, flow: string, label: string, required: readonly string[]): boolean {
  if (!object(value, flow, label)) {
    return false;
  }
  const record = value;
  for (const key of required) {
    assertion(flow, `${label}.${key} present`, present(record[key]));
  }
  return required.every((key) => present(record[key]));
}

/** Assert a page's length matches what the remaining dataset can fill. */
export function pageLength(actual: number, expected: number, flow: string, label: string): void {
  assertion(flow, `${label} length ${expected}`, actual === expected);
}

/** Assert a created record raised the matching count by exactly one. */
export function countDelta(before: number, after: number, flow: string, label: string): void {
  assertion(flow, `${label} count +1`, after === before + 1);
}

/** Assert two reads of the same ordered collection agree. */
export function idempotentReplay(first: unknown, second: unknown, flow: string, label: string): void {
  assertion(flow, `${label} replay identical`, JSON.stringify(first) === JSON.stringify(second));
}

/** Read a dotted path out of a value, if it exists. */
export function at(value: unknown, path: string): unknown {
  let current: unknown = value;
  for (const key of path.split(".")) {
    if (typeof current !== "object" || current === null) {
      return undefined;
    }
    current = (current as Record<string, unknown>)[key];
  }
  return current;
}

/**
 * Assert a representation reports the consequence a published event would
 * announce, for example `track_count` after a track is appended.
 */
export function sideEffect(value: unknown, flow: string, path: string, expected: unknown): void {
  assertion(flow, `side effect ${path}=${String(expected)}`, at(value, path) === expected);
}

/** Assert a numeric field is at least `minimum`. */
export function atLeast(value: unknown, minimum: number, flow: string, label: string): void {
  assertion(flow, `${label} >= ${minimum}`, typeof value === "number" && value >= minimum);
}

/** Assert a string is a HAL link with a non-empty `href`. */
export function link(value: unknown, flow: string, label: string): boolean {
  if (!fields(value, flow, label, ["href"])) {
    return false;
  }
  return present(at(value, "href"));
}
