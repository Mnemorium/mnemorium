---
name: glossary
description: Use when authoring or reviewing OpenSpec artifacts, specifications, design docs, or tasks, or when checking domain or technical terminology and wording consistency in this repository.
---

Use the repository's existing terms before creating new ones. `docs/development/Glossary.md` is the single
canonical glossary; it holds both business and technical terms in one `Term | Definition` table.

## Workflow

1. Identify the artifact being authored or reviewed: an OpenSpec proposal, design, tasks list, or delta spec, or
   another document under `docs/`.
2. Read `docs/development/Glossary.md`. For the change's domain, also read the relevant parts of
   `docs/development/Overview.md`, `docs/development/TechnicalDesign.md`, `docs/development/UseCases.md`,
   `docs/Permissions.md`, and `openspec/specs/<capability-path>/spec.md`.
3. Extract the project-specific, technical, overloaded, ambiguous, or repeated concepts the artifact introduces or
   uses.
4. Compare those concepts with the existing glossary terms.
5. If an existing term looks close, ask the user which to use and wait for the answer.
6. Use the chosen term consistently in the artifact.
7. Add each genuinely new term to `docs/development/Glossary.md`.
8. For a non-glossary artifact, create or update its companion glossary reference.

Add terms the project defines or overloads. Do not add generic dictionary words or one-off implementation details.

## Glossary File

`docs/development/Glossary.md` uses the existing two-column table:

```markdown
| Term | Definition |
| --- | --- |
```

Write one concise sentence per definition. Put the "use when" idea in the definition's opening clause, and state what
the term is not when a nearby term could be confused — `Application use case` already does this with
`Business use case`. A definition describes the concept, not the rule that governs it: never cite a rule identifier
(`REQ-…`, `DOC-…`, `OBS-…`) and never restate a rule's accepted values, configuration keys or build-failure
consequences; those stay in the rule and its capability specification. Keep the file's existing feature-area order:
append a new term beside the terms it relates to, or at the end. Do not create `glossary/business.md`,
`glossary/technical.md`, or a second glossary.

## Close-Term Question

Ask before creating a new term when existing terms may fit:

```text
These glossary terms look close to "<concept>":

- `<existing-term>`: <why it may fit>
- `<another-term>`: <why it may fit>

Do you want to use one of these existing terms, or define a new term?
```

Do not proceed with a new term until the user answers, unless no plausible existing term exists.

## Marking Terms in Artifacts

In a non-glossary artifact, bold a glossary term in prose when it appears in that artifact's companion glossary
reference. Do not bold terms in code blocks, frontmatter, links, tables, or places where markdown formatting would make
the artifact harder to read.

## Companion Reference

Every authored or edited non-glossary artifact gets a companion file in the same directory. Do not create one for
`docs/development/Glossary.md`.

Name it by removing `.md` and appending `-glossary-reference.md`:

- `proposal-glossary-reference.md`
- `design-glossary-reference.md`
- `tasks-glossary-reference.md`
- `spec-glossary-reference.md` beside each delta `spec.md`

For an OpenSpec change, the first three sit at the change root and the spec companions sit beside their `spec.md`, for
example `openspec/changes/<change>/specs/music/spec-glossary-reference.md`. Archived changes carry their companions into
the archive; main specs do not get companion files.

Format:

```markdown
# Glossary Reference

| Term | Source Glossary | Context |
| --- | --- | --- |
| <Term> | `docs/development/Glossary.md` | <Short context for how the artifact uses this glossary term.> |
```

If no glossary terms are used, write:

```markdown
# Glossary Reference

No glossary terms referenced.
```

Do not copy definitions into companion files.
