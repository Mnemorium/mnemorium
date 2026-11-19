# Glossary Reference

| Term                   | Source Glossary                | Context                                                                                            |
| ---------------------- | ------------------------------ | -------------------------------------------------------------------------------------------------- |
| Seed profile           | `docs/development/Glossary.md` | A setting resolved in three layers selects the fixture a `test-seed` boot applies.                 |
| Stored file            | `docs/development/Glossary.md` | The collection and the ingest's resource write are what make a stored file reachable over HTTP.    |
| Library root           | `docs/development/Glossary.md` | Every seeded and scanned file is addressed by a path relative to the root, never absolutely.       |
| Upload session         | `docs/development/Glossary.md` | A case owns its session and staged content; a whole-directory claim keeps a container.             |
| Chunk                  | `docs/development/Glossary.md` | A session's fixed chunk size is a startup setting, which keeps its case own.                       |
| Staged content         | `docs/development/Glossary.md` | A promoted upload becomes a stored file; a leftover is reclaimed at boot.                          |
| Integrity hash         | `docs/development/Glossary.md` | A completion recomputes it, and a transferred file may lose it to avoid a collision.               |
| Caller                 | `docs/development/Glossary.md` | Every listing and read is decided for the caller; no design decision restates who holds an action. |
| Resource               | `docs/development/Glossary.md` | The ingest now writes the file resource, and each seeded row's resource mirrors its use case.      |
| Authorization parent   | `docs/development/Glossary.md` | Per-item discoverability follows the file's parent chain, as the other listings decide theirs.     |
| Visibility             | `docs/development/Glossary.md` | Seeded library rows are public and seeded playlists private; the profile states each once.         |
| Catalogue              | `docs/development/Glossary.md` | The identifier band groups each module's catalogue rows under one block.                           |
| Cover art              | `docs/development/Glossary.md` | Public seeded files exist so the cover-art module can follow a cover link.                         |
| Security event         | `docs/development/Glossary.md` | The sink cases prove which catalogued event an outcome records and at which severity.              |
| Filter directives      | `docs/development/Glossary.md` | The severity ladder proves the boundary an installed directive draws.                              |
| Non-suppressible event | `docs/development/Glossary.md` | The inclusion cases prove a floor no directive can silence.                                        |
| Security-event catalog | `docs/development/Glossary.md` | The logging module's shared cases assert catalogue records by an owned value.                      |
| Root Admin             | `docs/development/Glossary.md` | A scanned file belongs to the Root Admin, and only the root rotates a credential.                  |
| Typed asset            | `docs/development/Glossary.md` | A file's typed row is what the item's link names, and the seed mirrors the probe's output.         |
| Asset scope            | `docs/development/Glossary.md` | Seeded images are library-scoped or user-scoped according to their first attachment.               |
| Partial representation | `docs/development/Glossary.md` | The seeded collections keep the partial item shape the API declares.                               |
| Default album          | `docs/development/Glossary.md` | The catalogue's protected seeded album; the glossary gains the term with this change.              |
| Default group          | `docs/development/Glossary.md` | The catalogue's protected seeded group; the glossary gains the term with this change.              |
| Instance state         | `docs/development/Glossary.md` | The pepper the seed hashes with is instance state, never configuration.                            |
| Action                 | `docs/development/Glossary.md` | The design names which actions a listing or read decides and leaves the who-may rule elsewhere.    |
