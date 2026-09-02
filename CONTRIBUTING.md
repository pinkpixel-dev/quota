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
cargo test --manifest-path src-tauri/Cargo.toml
```

## Building Installers

To produce the installers for the current version:

```bash
npm run build:release
```

This checks that the version fields agree, type checks, runs the Rust tests, bundles every target Tauri can build on your platform, lists what came out, and rewrites `SHA256SUMS.txt`.

Add `--skip-tests` for a faster rebuild while you are iterating, or `--skip-checksums` to leave the manifest alone. Anything after a bare `--` goes to the Tauri CLI:

```bash
npm run build:release -- --skip-tests --bundles deb
```

Tauri's `.deb` and `.rpm` output is not byte-reproducible, so any rebuild changes the checksums. If you use `--skip-checksums` and then ship, run `npm run checksums` first or the manifest will not match the files.

Windows installers have to be built on Windows through the GitHub Actions workflow. Once you have copied them into `src-tauri/target/release/bundle/`, run `npm run checksums` to record every installer in one manifest without rebuilding anything.

If an AppImage build fails or the resulting AppImage misbehaves on another distribution, read `APPIMAGE_FIX.md` before changing the build. It documents the failures this project has actually hit and what fixed each one.
