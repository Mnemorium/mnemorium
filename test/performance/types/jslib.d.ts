// Ambient declarations for the modules the generated client imports from
// `jslib.k6.io`. k6 resolves them at run time; `tsc` needs a shape to check
// against, and the generated client only uses `URL` and `URLSearchParams`.

declare module "https://jslib.k6.io/url/1.0.0/index.js" {
  export class URL {
    constructor(url: string, base?: string);
    search: string;
    pathname: string;
    toString(): string;
  }

  export class URLSearchParams {
    constructor(init?: string | Record<string, string> | Array<[string, string]>);
    append(name: string, value: string): void;
    toString(): string;
  }
}
