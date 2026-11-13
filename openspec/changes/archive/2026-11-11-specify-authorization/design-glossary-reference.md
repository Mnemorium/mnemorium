# Glossary Reference

| Term                 | Source Glossary                | Context                                                               |
| -------------------- | ------------------------------ | --------------------------------------------------------------------- |
| Resource             | `docs/development/Glossary.md` | The row the resource table stores and the decision addresses.         |
| Resource kind        | `docs/development/Glossary.md` | The kind that addresses a resource and names a `CreateChild` target.  |
| Authorization parent | `docs/development/Glossary.md` | The single edge the chain walks and the cycle/depth check guards.     |
| Resource relation    | `docs/development/Glossary.md` | The edges that lend read access but never inherit a full role.        |
| Principal            | `docs/development/Glossary.md` | The user, role or root a grant names.                                 |
| Grant                | `docs/development/Glossary.md` | The record the grant table stores and the decision resolves.          |
| Resource role        | `docs/development/Glossary.md` | The bundle a grant carries and the decision expands to actions.       |
| Owner                | `docs/development/Glossary.md` | The nearest non-null owner the chain resolves.                        |
| Visibility           | `docs/development/Glossary.md` | The three tiers the decision treats as an implicit allow.             |
| Denial               | `docs/development/Glossary.md` | The `404`/`403` split and the one-event rule.                         |
| Explanation          | `docs/development/Glossary.md` | The decision result the design threads through the application layer. |
| Action               | `docs/development/Glossary.md` | The generic catalog the design replaces the per-family verbs with.    |
| Root Admin           | `docs/development/Glossary.md` | The break-glass principal that precedes every deny.                   |
| Admin                | `docs/development/Glossary.md` | The role whose catalogue and account authority the design bounds.     |
| Caller               | `docs/development/Glossary.md` | The authenticated actor the decision is made for.                     |
| System               | `docs/development/Glossary.md` | The service under design whose behavior the decisions fix.            |
| Catalogue            | `docs/development/Glossary.md` | The unowned container the seed grants scope the Admin to.             |
| Creation surface     | `docs/development/Glossary.md` | The four collection resources decision 15 attaches a grant to.        |
