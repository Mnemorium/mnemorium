# Test

## Unit test

- The test module lives in the same file as the unit under test.
- When a unit needs an external dependency, mock it with `mockall`'s
  `#[automock]` attribute.
- Unit tests cover the HTTP handler and every layer except the outbound port
  adapters (SQLite, moka cache), which are exercised elsewhere.
- Do not test the payload DTOs (`<Context>Request`, `<Context>Query`,
  `<Context>Response`) in isolation; test the handler that consumes them.
- Repository query filters must be tested for strictness: the same filter backs
  both the SQLite repository and the moka cache, for example, and they must
  surface the same error.

### HTTP handler strategy

Test must:

- be `async`;
- send requests through `.oneshot(Request::builder()...)` from
  `tower::ServiceExt`;
- have one test per valid request variant, when the endpoint accepts several;
- have one test per invalid input attribute — invalid type, and invalid value
  depending on the type (for example, a number larger than the allowed bound);
- have one test per authorization rule;
- mock the application service / use case.

When one test covers two rules at once it is acceptable, but it must be
documented. Any test that falls outside the scope above (rare situation, cheap
coverage) must be justified in the documentation.

### Application strategy

Test must:

- be `async`;
- have one or more happy-path tests;
- test the domain models here — they have no dedicated test of their own;
- cover business-rule violations;
- cover validation errors;
- cover dependency failures.

Any test that falls outside the scope above (rare situation, cheap coverage)
must be justified in the documentation.
