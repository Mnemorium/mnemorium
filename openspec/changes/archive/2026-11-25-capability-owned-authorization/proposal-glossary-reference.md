# Glossary Reference

| Term                 | Source Glossary                | Context                                                                                                                                       |
| -------------------- | ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------- |
| Who-may rule         | `docs/development/Glossary.md` | The ownership boundary this proposal reverses: the capability owning the resource states the binding, and `authorization` states the meaning. |
| Resource             | `docs/development/Glossary.md` | The authorizable entity each capability declares a parent for.                                                                                |
| Authorization parent | `docs/development/Glossary.md` | The resource edge the capability-owned requirements declare.                                                                                  |
| Resource relation    | `docs/development/Glossary.md` | The read-lending edge the music capability owns; it never becomes an authorization parent.                                                    |
| Catalogue            | `docs/development/Glossary.md` | The unowned container whose children `movies`, `series` and `music` own.                                                                      |
| Creation surface     | `docs/development/Glossary.md` | The surface a `CreateChild` grant attaches to; the capability owns its default seed.                                                          |
| Action               | `docs/development/Glossary.md` | The operation each capability binds to its own routes and use cases.                                                                          |
| Resource role        | `docs/development/Glossary.md` | The bundle a capability's default creation grant carries.                                                                                     |
| Visibility           | `docs/development/Glossary.md` | The grant `is_public` selects for a gallery or playlist.                                                                                      |
| Owner                | `docs/development/Glossary.md` | The implicit role a capability's resource facts resolve.                                                                                      |
| Grant                | `docs/development/Glossary.md` | The stored record whose validation and resolution stay with `authorization`.                                                                  |
| Principal            | `docs/development/Glossary.md` | The user, role or root a grant names.                                                                                                         |
| Child kind           | `docs/development/Glossary.md` | The kind a capability's `CreateChild` binding names.                                                                                          |
| Typed asset          | `docs/development/Glossary.md` | The image, audio or video whose shared parent chain stays with the authorization engine.                                                      |
