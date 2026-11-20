# Glossary Reference

| Term              | Source Glossary                | Context                                                                                                               |
| ----------------- | ------------------------------ | --------------------------------------------------------------------------------------------------------------------- |
| Custom check      | `docs/development/Glossary.md` | The tasks add the two checks as pure functions with thin registrations; the glossary gains the term with this change. |
| Fuzz carve-out    | `docs/development/Glossary.md` | The final task audits every remaining line against the carve-out rule; the glossary gains the term with this change.  |
| Login identifier  | `docs/development/Glossary.md` | The login slice renames the request field and resolves username first; the glossary gains the term with this change.  |
| Closed vocabulary | `docs/development/Glossary.md` | The vocabulary slice publishes every enforced list as an enum schema; the glossary gains the term with this change.   |
| Security event    | `docs/development/Glossary.md` | The failed-login slice reduces the `authn_failed` event to its reason and removes the identity rule.                  |
