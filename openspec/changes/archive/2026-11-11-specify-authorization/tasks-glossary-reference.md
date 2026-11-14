# Glossary Reference

| Term                 | Source Glossary                | Context                                                                  |
| -------------------- | ------------------------------ | ------------------------------------------------------------------------ |
| Resource             | `docs/development/Glossary.md` | The row each test and migration task creates or resolves.                |
| Resource kind        | `docs/development/Glossary.md` | The kind each resource test addresses and each `CreateChild` test names. |
| Authorization parent | `docs/development/Glossary.md` | The edge the traversal and cycle tests prove.                            |
| Resource relation    | `docs/development/Glossary.md` | The edges the relation tests prove lend read but never edit.             |
| Principal            | `docs/development/Glossary.md` | The user, role or root each permission test grants to.                   |
| Grant                | `docs/development/Glossary.md` | The record the permission API tests read, replace and revoke.            |
| Resource role        | `docs/development/Glossary.md` | The bundle each decision test holds.                                     |
| Owner                | `docs/development/Glossary.md` | The implicit role the ownership tests resolve.                           |
| Visibility           | `docs/development/Glossary.md` | The tier each visibility test sets.                                      |
| Action               | `docs/development/Glossary.md` | The catalog action each `feature` label and call-site task names.        |
| Child kind           | `docs/development/Glossary.md` | The target kind each creation test names.                                |
| Denial               | `docs/development/Glossary.md` | The `404`/`403` and one-event assertions.                                |
| Explanation          | `docs/development/Glossary.md` | The decision detail the explanation tests read.                          |
| Root Admin           | `docs/development/Glossary.md` | The caller the root, upload and account tests authenticate as.           |
| Admin                | `docs/development/Glossary.md` | The role the account and catalogue tests provision.                      |
| Standard User        | `docs/development/Glossary.md` | The unprivileged account the standard tests provision.                   |
| Caller               | `docs/development/Glossary.md` | The actor each system test drives.                                       |
| System               | `docs/development/Glossary.md` | The service under test.                                                  |
| Security event       | `docs/development/Glossary.md` | The `authz_failed` event the denial tests assert exactly once.           |
| Content Type         | `docs/development/Glossary.md` | The declared type left to Schemathesis rather than re-asserted.          |
| Catalogue            | `docs/development/Glossary.md` | The container the tree test and the seeded grants resolve.               |
| Creation surface     | `docs/development/Glossary.md` | The surface the 8.5 seed grants attach to and scenario 023-E proves.     |
