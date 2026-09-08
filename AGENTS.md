# AGENTS.md

## Introduction

Mnemorium is a self-hosted media server designed for home use.

This repository is the **server backend** of a client-server architecture: it
exposes the whole application to clients through a REST API. It is written in
Rust, uses **axum** for HTTP and **sqlx + SQLite3** as the single datastore.

Design goals:

- One Docker container ships the entire backend — no external services.
- REST-first: clients talk to the server exclusively over HTTP.

## Style & Conventions

- Rust and SQL conventions and guidelines live in `docs/development/StyleGuide.md`.

## Use Cases

- The catalog of use cases and their conventions lives in
  `docs/development/UseCases.md`; each use case maps to one file in
  `src/lib/application/use_case/`.

## API

- REST and OpenAPI guidelines live in `docs/development/api/Overview.md`.
- The OpenAPI specification is generated from source to `docs/development/api/openapi.json`; regenerate it with `cargo run --bin openapi_gen` (see `Overview.md` for details).

## Tests

- Testing guidelines, strategies, and conventions live in
  `docs/development/Test.md`.
