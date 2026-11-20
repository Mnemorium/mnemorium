# Glossary Reference

| Term              | Source Glossary                | Context                                                                                                               |
| ----------------- | ------------------------------ | --------------------------------------------------------------------------------------------------------------------- |
| Closed vocabulary | `docs/development/Glossary.md` | The design binds each enforced wire vocabulary to a handler-local enum; the glossary gains the term with this change. |
| Fuzz carve-out    | `docs/development/Glossary.md` | The design gives the exemption rule two categories; the glossary gains the term with this change.                     |
| Custom check      | `docs/development/Glossary.md` | The two Schemathesis checks are pure functions with thin registrations; the glossary gains the term with this change. |
| Login identifier  | `docs/development/Glossary.md` | The design renames the request field and resolves username first; the glossary gains the term with this change.       |
| Action            | `docs/development/Glossary.md` | The permission summary query publishes the action catalog as an enum.                                                 |
| Resource kind     | `docs/development/Glossary.md` | A permission parameter and a grant's child kind publish a resource kind.                                              |
| Resource role     | `docs/development/Glossary.md` | A share and a grant publish the resource roles they accept.                                                           |
| Child kind        | `docs/development/Glossary.md` | A grant publishes the grantable child kinds, excluding `Admin`.                                                       |
| Principal         | `docs/development/Glossary.md` | A grant and a share name a principal kind.                                                                            |
| Visibility        | `docs/development/Glossary.md` | A permission body publishes the visibilities it accepts.                                                              |
| Caller            | `docs/development/Glossary.md` | The name, login and collection rules act for a caller.                                                                |
| Stored file       | `docs/development/Glossary.md` | A content response publishes a stored file's bytes as a binary string.                                                |
| Security event    | `docs/development/Glossary.md` | `authn_failed` loses its identity field; the other catalogued events keep their shape.                                |
| Root Admin        | `docs/development/Glossary.md` | A grant's `Admin` child kind is root-only and excluded from the published vocabulary.                                 |
