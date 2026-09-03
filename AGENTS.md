# AGENTS.md

## Introduction

Mnemorium is a self-hosted media server designed for home use.

This repository is the **server backend** of a client-server architecture: it
exposes the whole application to clients through a REST API. It is written in
Rust, uses **axum** for HTTP and **sqlx + SQLite3** as the single datastore.

Design goals:

- One Docker container ships the entire backend — no external services.
- REST-first: clients talk to the server exclusively over HTTP.

The rest of this file (architecture, conventions, commands) is a work in
progress.
