## [0.6.6](https://github.com/Mnemorium/mnemorium/compare/v0.6.5...v0.6.6) (2026-10-04)

### Bug Fixes

* **domain:** declare asset upload port traits send and sync ([#155](https://github.com/Mnemorium/mnemorium/issues/155)) ([9d993f1](https://github.com/Mnemorium/mnemorium/commit/9d993f195a5058153e0fd6c81cb1365cb78e93f1))

## [0.6.5](https://github.com/Mnemorium/mnemorium/compare/v0.6.4...v0.6.5) (2026-10-04)

### Bug Fixes

* **persistence:** drop the genre upper-id trigger by its real name ([#145](https://github.com/Mnemorium/mnemorium/issues/145)) ([f44232c](https://github.com/Mnemorium/mnemorium/commit/f44232c79b2146b94e4b129f148906a503d5bdf6))

## [0.6.4](https://github.com/Mnemorium/mnemorium/compare/v0.6.3...v0.6.4) (2026-10-04)

### Bug Fixes

* **api:** return 422 for validation failures and document the status taxonomy ([#132](https://github.com/Mnemorium/mnemorium/issues/132)) ([93b8e80](https://github.com/Mnemorium/mnemorium/commit/93b8e80a8685a777edf2eb9c8d261e42d8649f34))

## [0.6.3](https://github.com/Mnemorium/mnemorium/compare/v0.6.2...v0.6.3) (2026-10-03)

### Bug Fixes

* **api:** prevent username enumeration on login ([#138](https://github.com/Mnemorium/mnemorium/issues/138)) ([67ba916](https://github.com/Mnemorium/mnemorium/commit/67ba916ead78fd2a37d20869720ef6be55b3c5ea))

## [0.6.2](https://github.com/Mnemorium/mnemorium/compare/v0.6.1...v0.6.2) (2026-10-03)

### Bug Fixes

* **api:** log only classified request and auth rejections ([#134](https://github.com/Mnemorium/mnemorium/issues/134)) ([56168a0](https://github.com/Mnemorium/mnemorium/commit/56168a085230f191628a7c3d4a0c50984bd1cc9e))

## [0.6.1](https://github.com/Mnemorium/mnemorium/compare/v0.6.0...v0.6.1) (2026-10-03)

### Bug Fixes

* **api:** sanitize JSON extractor rejections and map 415/422 ([#127](https://github.com/Mnemorium/mnemorium/issues/127)) ([3600972](https://github.com/Mnemorium/mnemorium/commit/36009726c0d66e6ae45ba39f0cdd3532f702a6e7))

## [0.6.0](https://github.com/Mnemorium/mnemorium/compare/v0.5.2...v0.6.0) (2026-10-03)

### Features

* **api:** implement the list users endpoint ([#108](https://github.com/Mnemorium/mnemorium/issues/108)) ([1af11a5](https://github.com/Mnemorium/mnemorium/commit/1af11a56e953a3ac368fd7b33d1afc50c1ccf682))

### Bug Fixes

* **infrastructure:** model a missing file as a value, not a port error ([#109](https://github.com/Mnemorium/mnemorium/issues/109)) ([45e5780](https://github.com/Mnemorium/mnemorium/commit/45e57807bc6ce4ab604588391c6427e68d1bad0a))

## [0.5.2](https://github.com/Mnemorium/mnemorium/compare/v0.5.1...v0.5.2) (2026-10-03)

### Bug Fixes

* **api:** add self links to user resource representations ([#69](https://github.com/Mnemorium/mnemorium/issues/69)) ([1110840](https://github.com/Mnemorium/mnemorium/commit/111084003954d5824b160a40d838d7e2eeafaaca))
* **infrastructure:** fail startup on invalid persistence settings ([#102](https://github.com/Mnemorium/mnemorium/issues/102)) ([d89c449](https://github.com/Mnemorium/mnemorium/commit/d89c449f4305d79939534b8e08f4569f78815947))

## [0.5.1](https://github.com/Mnemorium/mnemorium/compare/v0.5.0...v0.5.1) (2026-10-03)

### Bug Fixes

* **domain:** validate configuration deserialization ([#103](https://github.com/Mnemorium/mnemorium/issues/103)) ([9b8392b](https://github.com/Mnemorium/mnemorium/commit/9b8392bd85620b40b56f421379771e7e3ec2da54))

## [0.5.0](https://github.com/Mnemorium/mnemorium/compare/v0.4.1...v0.5.0) (2026-10-03)

### Features

* **agent:** add investigate command ([0404d85](https://github.com/Mnemorium/mnemorium/commit/0404d859b064f7c591eefecaf8d33ec2158f1989))

## [0.4.1](https://github.com/Mnemorium/mnemorium/compare/v0.4.0...v0.4.1) (2026-10-02)

### Bug Fixes

* **persistence:** map SQLite trigger aborts to RepositoryError::Conflict ([#89](https://github.com/Mnemorium/mnemorium/issues/89)) ([5e3911a](https://github.com/Mnemorium/mnemorium/commit/5e3911a7a7b322f7c264b3762734b9391576392e))

## [0.4.0](https://github.com/Mnemorium/mnemorium/compare/v0.3.1...v0.4.0) (2026-10-02)

### Features

* **api:** chunked upload ([#31](https://github.com/Mnemorium/mnemorium/issues/31)) ([a56e441](https://github.com/Mnemorium/mnemorium/commit/a56e4411b9a4a2ee678cbda1a3efb073db04371b))

## [0.3.1](https://github.com/Mnemorium/mnemorium/compare/v0.3.0...v0.3.1) (2026-10-01)

### Bug Fixes

* **api:** declare explicit security for public endpoints ([#48](https://github.com/Mnemorium/mnemorium/issues/48)) ([65e9e5a](https://github.com/Mnemorium/mnemorium/commit/65e9e5aa2b51e424f156f53c382322a6a48c7c41))

## [0.3.0](https://github.com/Mnemorium/mnemorium/compare/v0.2.0...v0.3.0) (2026-09-29)

### Features

* **agent:** add system-architect subagent ([32340d7](https://github.com/Mnemorium/mnemorium/commit/32340d7f94fc63cc0c749865acb7e3b018f362da))

### Bug Fixes

* **agent:** parse system-architect frontmatter ([81e1b85](https://github.com/Mnemorium/mnemorium/commit/81e1b856888d5e1f91fe80d085e2d54a777365f9))

## [0.2.0](https://github.com/Mnemorium/mnemorium/compare/v0.1.8...v0.2.0) (2026-09-26)

### Features

* **application:** add configurable logging settings ([#21](https://github.com/Mnemorium/mnemorium/issues/21)) ([d30aa45](https://github.com/Mnemorium/mnemorium/commit/d30aa45b29fa866ae9ae35c6008f7666cd8a8d5a))

## [0.1.8](https://github.com/Mnemorium/mnemorium/compare/v0.1.7...v0.1.8) (2026-09-25)

### Bug Fixes

* **application:** fix the error handling in token-provider ([#17](https://github.com/Mnemorium/mnemorium/issues/17)) ([1c0ed0d](https://github.com/Mnemorium/mnemorium/commit/1c0ed0de142f8848790db618a0d1caf83c014c31))

## [0.1.7](https://github.com/Mnemorium/mnemorium/compare/v0.1.6...v0.1.7) (2026-09-13)

### Bug Fixes

* **api:** change media to mime ([#7](https://github.com/Mnemorium/mnemorium/issues/7)) ([dd723b0](https://github.com/Mnemorium/mnemorium/commit/dd723b088c5d50e49007960a9d3cf1241f38d929))
* **api:** remove media ([#8](https://github.com/Mnemorium/mnemorium/issues/8)) ([70b4c85](https://github.com/Mnemorium/mnemorium/commit/70b4c8527c8e495cd810becdae4faa4db5d28807))
