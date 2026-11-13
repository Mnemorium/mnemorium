# Glossary Reference

| Term           | Source Glossary                | Context                                                                                           |
| -------------- | ------------------------------ | ------------------------------------------------------------------------------------------------- |
| Caller         | `docs/development/Glossary.md` | The session and the completed file belong to the caller who began the upload.                     |
| Chunk          | `docs/development/Glossary.md` | One addressed piece of a session's content, written at its declared offset.                       |
| Content Type   | `docs/development/Glossary.md` | The declared MIME type; the probe confirms it and never replaces it.                              |
| Integrity hash | `docs/development/Glossary.md` | The whole-file digest declared at begin and recomputed at completion.                             |
| Library root   | `docs/development/Glossary.md` | The operator's mount the promoted file is stored under; the server never creates it.              |
| Owner          | `docs/development/Glossary.md` | The completed file belongs to its uploader; the access rule itself is `identity/authorization`'s. |
| Staged content | `docs/development/Glossary.md` | The bytes a session holds under the server-owned staging root before promotion.                   |
| Upload session | `docs/development/Glossary.md` | The resumable, caller-owned unit whose lifecycle this capability owns.                            |
| Visibility     | `docs/development/Glossary.md` | A completed upload produces a `Private` file; the meaning stays with `identity/authorization`.    |
