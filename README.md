# codex

[![Lint](https://github.com/kahlys/codex/actions/workflows/lint.yml/badge.svg)](https://github.com/kahlys/codex/actions/workflows/lint.yml)
[![Tests](https://github.com/kahlys/codex/actions/workflows/ci.yml/badge.svg)](https://github.com/kahlys/codex/actions/workflows/ci.yml)

Personal collection of tools, libraries, projects, and proof-of-concepts.

## Tools

### Go CLI Tools

| Name | Description |
|------|-------------|
| [basespy](tools/go/basespy) | Decode base32/base64 encoded strings and reveal hidden data |
| [bazaar](tools/go/bazaar) | Docker container orchestration and management tool |
| [cemetery](tools/go/cemetery) | Query end-of-life dates for software and platforms via endoflife.date API |
| [geninter](tools/go/geninter) | Generate Go interfaces from existing types |
| [oracle](tools/go/oracle) | Credential extraction and management tool |
| [osv-viewer](tools/go/osv-viewer) | Display and analyze osv-scanner JSON output for vulnerabilities |
| [pika](tools/go/pika) | Generate PKI (Public Key Infrastructure) for development and testing |
| [proxy](tools/go/proxy) | Simple TCP proxy for network forwarding |
| [wanda](tools/go/wanda) | Generate REST clients and servers from OpenAPI specifications |

### Rust CLI Tools

| Name | Description |
|------|-------------|
| [baywatch](tools/rust/baywatch) | Profile containerized workloads with various CPU configurations |
| [lifegame](tools/rust/lifegame) | Conway's Game of Life implementation |

## Libraries

### Go Packages

| Name | Description |
|------|-------------|
| [alfred](libraries/go/alfred) | Filter and sort Go slices of structs with flexible options |
| [psqlx](libraries/go/psqlx) | Wrapper around pgx PostgreSQL driver with specific error handling |

## Projects

| Name | Description |
|------|-------------|
| [blobtrack](projects/blobtrack) | Interactive blob tracking visualizer (Web application) |

## Proof of Concepts

### Rust Mini Projects

| Name | Description |
|------|-------------|
| [crud](poc/rust/mini/crud) | Basic CRUD API with authentication and database |
| [db](poc/rust/mini/db) | Database experimentation |
| [ex](poc/rust/mini/ex) | Learning experiments |
| [grep](poc/rust/mini/grep) | Text search utility implementation |
| [tcphello](poc/rust/mini/tcphello) | TCP server hello world |
