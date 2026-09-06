# Test

## Unit test

- The test module lives in the same file as the unit under test.
- When a unit needs an external dependency, mock it with `mockall`'s
  `#[automock]` attribute.
- Unit tests cover the HTTP handler and every layer except the outbound port
  adapters (SQLite, moka cache), which are exercised in the
  [Integration test](#integration-test) section.
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
- test the domain models here — **they have no dedicated test of their own**
  (domain services _do_; see below);
- cover business-rule violations;
- cover validation errors;
- cover dependency failures.

Any test that falls outside the scope above (rare situation, cheap coverage)
must be justified in the documentation.

### Domain service strategy

- Domain services (in `src/lib/domain/service/`) have a dedicated unit test
  module of their own, living in the same file as the service under test.
- Test must cover the business rules each service enforces.
- Domain services are pure — no external dependency, so no mocking.

## Integration test

- Integration tests exercise the outbound port adapters: the repository (SQLite,
  moka cache) and the external services.
- The test module lives in the same file as the code under test.

### Repository

Test must:

- have one happy-path test;
- verify every constraint — declared in the db schema and reflected in the rest
  of the project;
- test every trigger defined in the schema.

When one test covers two rules at once it is acceptable, but it must be
documented. Any test that falls outside the scope above (rare situation, cheap
coverage) must be justified in the documentation.

#### SQLite3

- Use an in-memory database — one `:memory:` connection per test, so each test
  runs against a fresh database.
- Configure sqlx with the SQLite option that caps the pool at one connection.

#### Moka

- Mock the wrapped repository with `mockall`.

### External Service

- Stub the service with `wiremock`; canned responses come from golden files.
- Test every possible server response — including failures such as `Timeout`.
- The test module lives in the same file as the client under test. Reformat the
  file when needed so the test module stays clear.

## E2E test

- Tests are written in Python with `pytest`.
- Tests live under `test/e2e/`.
- One file per use case, matching an entry in the
  [use case catalog](UseCases.md).
- One folder per bounded context, as listed in the
  [Bounded context section of the Overview](Overview.md#bounded-context).
