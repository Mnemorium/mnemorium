# Glossary Reference

| Term                 | Source Glossary                | Context                                                                   |
| -------------------- | ------------------------------ | ------------------------------------------------------------------------- |
| Resource             | `docs/development/Glossary.md` | The entity every requirement decides against.                             |
| Resource kind        | `docs/development/Glossary.md` | The kind that addresses a resource and names a `CreateChild` target.      |
| Authorization parent | `docs/development/Glossary.md` | The single edge the decision walks, capped at 32 and kept acyclic.        |
| Resource relation    | `docs/development/Glossary.md` | The edges that lend read access but are not authorization parents.        |
| Principal            | `docs/development/Glossary.md` | The user, role or root a grant names.                                     |
| Grant                | `docs/development/Glossary.md` | The record a principal holds on a resource, with an allow or deny effect. |
| Resource role        | `docs/development/Glossary.md` | The six bundles that map a grant to actions.                              |
| Owner                | `docs/development/Glossary.md` | The implicit role resolved from the nearest `owner_id`.                   |
| Visibility           | `docs/development/Glossary.md` | The `Private`, `Restricted` and `Public` tiers.                           |
| Action               | `docs/development/Glossary.md` | The thirteen-action catalog and its stored names.                         |
| Child kind           | `docs/development/Glossary.md` | The `ChildKind` a `CreateChild` action names.                             |
| Denial               | `docs/development/Glossary.md` | The `404`/`403` split and the one-event rule.                             |
| Explanation          | `docs/development/Glossary.md` | The decision detail every requirement returns.                            |
| Root Admin           | `docs/development/Glossary.md` | The implicit owner that bypasses every deny, except for uploads.          |
| Standard User        | `docs/development/Glossary.md` | The non-admin account with no administration grants.                      |
| Admin                | `docs/development/Glossary.md` | The role that curates content and manages Standard accounts.              |
| Caller               | `docs/development/Glossary.md` | The authenticated actor a decision is made for.                           |
| Security event       | `docs/development/Glossary.md` | The `authz_failed` event a denial emits.                                  |
| System               | `docs/development/Glossary.md` | The service under design.                                                 |
| Catalogue            | `docs/development/Glossary.md` | The unowned container for films, shows, albums and music groups.          |
| Creation surface     | `docs/development/Glossary.md` | The collection resources a seeded `CreateChild` grant attaches to.        |
