# Glossary Reference

| Term           | Source Glossary                | Context                                                                                                           |
| -------------- | ------------------------------ | ----------------------------------------------------------------------------------------------------------------- |
| Caller         | `docs/development/Glossary.md` | The proposal attributes a session and its completed file to the caller who began it.                              |
| Content Type   | `docs/development/Glossary.md` | The declared MIME type the uploader submits and the probe confirms; it is not reclassified from the content.      |
| Owner          | `docs/development/Glossary.md` | The completed file belongs to its uploader; the authorization rule that follows is not restated here.             |
| Visibility     | `docs/development/Glossary.md` | A completed upload produces a `Private` file; the meaning of that visibility stays with `identity/authorization`. |
| Upload session | `docs/development/Glossary.md` | The resumable, caller-owned unit the upload capability introduces.                                                |
| Chunk          | `docs/development/Glossary.md` | One addressed piece of a session's content, written at its declared offset.                                       |
| Staged content | `docs/development/Glossary.md` | The bytes a session holds under the server-owned staging root before promotion.                                   |
| Library root   | `docs/development/Glossary.md` | The operator's mount the completed file is promoted under; the server never creates it.                           |
| Integrity hash | `docs/development/Glossary.md` | The whole-file digest the uploader declares and completion recomputes.                                            |
