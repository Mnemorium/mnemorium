---
name: create-issue
description: Files one GitHub issue for a group of related pre-existing concerns surfaced by a pull request
  review. Formats a decision request with the AI disclosure and a needs-triage label, then calls the
  github-issue_create tool. Use when review-pr hands off pre-existing concerns that are not yet tracked.
---

# Create Issue

File one GitHub issue for one group of related pre-existing concerns.

## When to use

`review-pr` hands off pre-existing concerns that are not yet tracked in the issue tracker. The caller has
already removed the concerns that have an equivalent open issue; do not re-check for duplicates here.

## Inputs

- One group of related pre-existing concerns. Each concern has a location (`path:line`), the dimension it
  appears to breach, a fact and rationale, and optionally an authoritative source. When no rule covers the
  concern, the source states the potential rule gap instead.
- The pull request reference (`number` and `url`) where the concerns were found.

## Process

1. Compose the title as `<rule> in <module>` when a rule applies, for example `Constraint naming in migrations`.
   When no rule applies, use `<dimension> concern in <module>`.
2. Compose the body exactly as the "Pre-existing decision request" template in
   `docs/development/IssueTracking.md`: the AI disclosure first, then Context (the pull request reference and
   that the concern is pre-existing), then Evidence (one `path:line` entry per concern with its source or
   potential rule gap), then Decision.
3. Call the `github-issue_create` tool with the title, the body, and `labels: ["needs-triage"]`.
4. Return the created issue number and URL to the caller.

## Constraints

- File exactly one issue per invocation, for the group you were given.
- Cite the source attached to the concerns when there is one. Never invent a rule; when none applies, state the
  potential rule gap instead.
- Do not search for existing issues; deduplication is owned by `review-pr`.
- If the `github-issue_create` tool fails, report the failure and stop. Leave the concern in the review comment
  unfiled; do not retry or fall back to another mechanism.
- Do not edit the repository.
