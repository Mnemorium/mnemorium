# Glossary Reference

| Term                   | Source Glossary                | Context                                                                                               |
| ---------------------- | ------------------------------ | ----------------------------------------------------------------------------------------------------- |
| Log rotation           | `docs/development/Glossary.md` | The design keeps the existing clock-free way of proving rotation and pruning.                         |
| Filter directives      | `docs/development/Glossary.md` | The design places their validation in the capability that consumes the value.                         |
| Redaction              | `docs/development/Glossary.md` | The design moves the sink rule and keeps the stored-hash rule with its capability.                    |
| Security event         | `docs/development/Glossary.md` | The design makes the capability the single home of the catalog and moves the rulebook's parser to it. |
| Non-suppressible event | `docs/development/Glossary.md` | The design keeps the floor and proves it through a quiet-verbosity scenario.                          |
| Log injection          | `docs/development/Glossary.md` | The design keeps the escaping requirement and proves it through a rejected login.                     |
