# Glossary Reference

| Term                   | Source Glossary                | Context                                                                                 |
| ---------------------- | ------------------------------ | --------------------------------------------------------------------------------------- |
| Log rotation           | `docs/development/Glossary.md` | The file sink's replacement period and the pruning bound the capability makes testable. |
| Filter directives      | `docs/development/Glossary.md` | The `logging.level` value the capability validates at startup.                          |
| Redaction              | `docs/development/Glossary.md` | The requirement that no secret or personal value reaches a sink.                        |
| Security event         | `docs/development/Glossary.md` | The catalogued events the capability now declares with their trigger, level and fields. |
| Non-suppressible event | `docs/development/Glossary.md` | The floor that keeps a catalogued event out of `logging.level`'s reach.                 |
| Log injection          | `docs/development/Glossary.md` | The bound and escaping the one client-supplied field carries.                           |
