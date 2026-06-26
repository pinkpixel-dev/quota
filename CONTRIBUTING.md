# Contributing to Quota

## Development Principles

- Keep the app small and understandable.
- Prefer the simplest solution that works.
- Keep secrets in Rust/backend-owned flows whenever possible.
- Update docs when changes are made.

## Local Setup

```bash
npm install
npm run dev
```

For the desktop shell:

```bash
npm run tauri dev
```

## Before Opening A Pull Request

Run:

```bash
npm run typecheck
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```
