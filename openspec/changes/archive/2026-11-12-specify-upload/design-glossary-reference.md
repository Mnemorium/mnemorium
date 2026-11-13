# Glossary Reference

| Term           | Source Glossary                | Context                                                                                             |
| -------------- | ------------------------------ | --------------------------------------------------------------------------------------------------- |
| Caller         | `docs/development/Glossary.md` | The completed file is attributed to the caller who began the session.                               |
| Content Type   | `docs/development/Glossary.md` | The declared MIME type is authoritative and is confirmed, never replaced, by the probe.             |
| Integrity hash | `docs/development/Glossary.md` | The whole-file digest declared at begin and recomputed at completion.                               |
| Library root   | `docs/development/Glossary.md` | The promoted file is stored under it; the server never creates it.                                  |
| Owner          | `docs/development/Glossary.md` | Ownership is recorded by `upload`; the access rule that follows is `identity/authorization`'s.      |
| Staged content | `docs/development/Glossary.md` | The bytes held under the staging root before promotion and after an interrupted begin.              |
| Upload session | `docs/development/Glossary.md` | The resumable unit whose lifecycle the capability owns.                                             |
| Visibility     | `docs/development/Glossary.md` | The promoted file is `Private`; the meaning of that visibility stays with `identity/authorization`. |
