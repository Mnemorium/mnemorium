# Proposal

## Why

The three typed media rows — image, audio and video — are implemented end to end, but their contract lives inside
`library/assets`, where file serving and intake once shared a home. The area holds 27 requirements, none carrying the
repository's `REQ-*`/`SCN-*` convention, and the typed rows appear only as route-level reads. Nothing states what an
image, an audio or a video _is_: which measurements are required, which may be absent, that an image's orientation is
derived from its dimensions, that a video carries no name, or that an audio carries no asset scope. The current text
even claims a video row carries the uploaded file name, which the schema cannot.

The redesign gives every functional area one owner with stable identifiers. `library/assets` is becoming the serving
capability — file read, content, ranges, validators and the content credential — so the three entity areas are the next
owners to split out.

## What Changes

- Add `image`, `audio` and `video` capabilities, each authored as one entity area with `REQ-IMAGE-*`, `REQ-AUDIO-*` and
  `REQ-VIDEO-*` identifiers.
- State each entity's characteristics as the contract:
  - an **image** carries a name, positive dimensions, an orientation derived from those dimensions, a creation date and
    an optional asset scope;
  - an **audio** carries a name and a codec, with duration, sample rate, bit depth and block size absent when the
    content does not report them, and no asset scope;
  - a **video** carries a codec, a colour identifier, duration, frame count, dimensions, scan type and an optional asset
    scope, and no name.
- State the representation and the `/file` alias each authenticated **caller** reads, including the summary shape a
  **resource** embeds for its video.
- Remove the four typed-read requirements and the media-row naming requirement from `library/assets`; it keeps the file
  read, content serving, ranges, validators, content credential and outcome distinction.
- Move the naming rule to `image` and `audio` as a characteristic and drop the false video clause; the intake
  capabilities keep the rule that chooses the name.
- No route, payload, status or schema changes. **BREAKING**: none.

## Capabilities

### New Capabilities

- `image`: the typed image row — its backing file, characteristics, representation and file alias.
- `audio`: the typed audio row — its backing file, characteristics, representation and file alias.
- `video`: the typed video row — its backing file, characteristics, representation, the summary other resources embed,
  and its file alias.

### Modified Capabilities

- `library/assets`: the typed image, audio and video requirements and the media-row naming requirement are removed; the
  serving requirements stay.

## Impact

- **Specs**: three capabilities are created; `library/assets` falls from 11 to 6 requirements after `specify-upload`
  lands, or from 27 to 22 before it.
- **API**: none. The routes, payloads, statuses and served OpenAPI document are unchanged.
- **Code**: none. The characteristics are already recorded by `domain/model/{image,audio,video}.rs` and served by
  `handler/asset/get_*`.
- **Tests**: `test/system/image.py`, `test/system/audio.py` and `test/system/video.py` become the black-box proof;
  `test/e2e/asset/uc031_read_asset_content.py` keeps only its business flow; the application tests for the typed reads
  gain the three Allure labels.
- **Docs**: `TechnicalDesign.md` § 5 names the three system modules; the glossary gains **Typed asset**, **Image**,
  **Audio**, **Video**, **Asset scope**, **Orientation** and **Scan type**, or reuses the Overview model terms where
  they already exist.
- **Sequencing**: authored against the tree after `specify-upload`, which rewrites `library/assets`' Purpose directly
  and deliberately leaves the naming rule in place. This change supersedes that decision and removes the rule.
