# Spec Delta

## Purpose

Lets a client read one stored audio file as a typed entity: the file it represents, the measurements the content reports
or leaves absent, and the link to its content.

## ADDED Requirements

### Requirement: REQ-AUDIO-001 — An audio is a typed sound file

An audio SHALL be created only from content the intake identifies as audio, and SHALL reference exactly one stored file.
A stored file SHALL back at most one audio.

#### Scenario: SCN-AUDIO-001-A — Audio content creates one audio

- **WHEN** an audio file completes intake
- **THEN** one audio exists, backed by the file the intake registered
- **AND** the file representation links that audio

#### Scenario: SCN-AUDIO-001-B — A file backs one audio

- **WHEN** the same stored file is registered again
- **THEN** no second audio is created for it

#### Scenario: SCN-AUDIO-001-C — Content of another media type is not an audio

- **WHEN** image or video content completes intake
- **THEN** no audio is created

### Requirement: REQ-AUDIO-002 — An audio reports its characteristics

An audio SHALL report a non-empty name and codec. It SHALL report a duration in milliseconds, a sample rate in hertz, a
bit depth and a block size when the content reports them. A measurement the content does not report SHALL be absent,
never fabricated.

#### Scenario: SCN-AUDIO-002-A — Every measurement is reported

- **WHEN** the content reports every measurement
- **THEN** the representation carries the name, codec, duration, sample rate, bit depth and block size

#### Scenario: SCN-AUDIO-002-B — The content reports no bit depth or block size

- **WHEN** the content reports no bit depth or block size
- **THEN** the representation carries that measurement as absent rather than a fabricated value

#### Scenario: SCN-AUDIO-002-C — The content reports no duration

- **WHEN** the content reports no duration or sample rate
- **THEN** the representation carries that measurement as absent rather than a fabricated value

### Requirement: REQ-AUDIO-003 — An audio is readable

A caller who may read an audio SHALL read it. The representation SHALL carry the audio identifier, name, codec, the
measurements the content reports and the backing file identifier. An unknown or unreadable audio SHALL answer
`404 Not Found`, indistinguishable from a missing one.

#### Scenario: SCN-AUDIO-003-A — An audio of an existing identifier

- **WHEN** an authenticated caller requests an audio it may read
- **THEN** the system returns `200 OK` with the audio representation

#### Scenario: SCN-AUDIO-003-B — Unknown or unreadable audio

- **WHEN** the audio does not exist, or the caller may not read it
- **THEN** the system returns `404 Not Found`

### Requirement: REQ-AUDIO-004 — An audio addresses its file

The system SHALL serve the file alias of an audio as an alias of the backing file resource, answering `303 See Other`
with a `Location` naming the file.

#### Scenario: SCN-AUDIO-004-A — The alias redirects

- **WHEN** an authenticated caller requests the file alias of an audio it may read
- **THEN** the system returns `303 See Other` with the file resource in `Location`
