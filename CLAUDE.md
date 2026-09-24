# ThoughtRouter — agent guide

Local-first desktop app for capturing messy thoughts and organizing them later. Read `README.md`
first; product intent lives in `docs/`, the build plan in `docs/MVP_PLAN.md`, decisions in `DECISIONS.md`.

## Layout

- `crates/core/` — all behaviour (Rust): `db.rs` + `migrations/`, `captures`, `atoms`, `jobs`,
  `pipeline` (job stages), `processor/` (`Analyzer`/`Embedder` traits, `mock`, `openrouter`),
  `embeddings`, `search`, `projects`, `resurface`, `export`, `stats`, `eval`.
- `src-tauri/` — thin Tauri command layer (`commands.rs`), keychain, startup, global hotkey.
- `src/` — React UI. `src/api.ts` wraps every command; `src/bindings/` is generated — never edit it.
- `fixtures/*.json` — golden fixtures; `crates/core/examples/eval.rs` runs them.

## Commands

```sh
cargo test -p thoughtrouter-core                         # core tests + regenerates src/bindings
cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check
npm test && npm run typecheck && npm run lint            # UI
npm run tauri dev                                        # run the app
cargo run -p thoughtrouter-core --example eval -- --mock # fixture eval
```

Building `src-tauri` needs the Tauri Linux system libraries (see README) and an existing `dist/`
(`npm run build`).

## Invariants (do not break)

1. `captures.text`, `source`, `captured_at` are immutable (SQLite trigger). Only `deleted_at` changes.
2. AI output is derived data. Reprocessing supersedes only `origin = 'ai'` atoms; `origin = 'user'`
   atoms and `status = 'rejected'` links survive and are never re-suggested (D-019).
3. Saving a capture never waits on AI; failures leave the capture intact and retryable (D-010).
4. Never hold `Db::conn()` across an `.await` (the guard is `!Send`, so this usually won't compile).
5. Projects are never created without a user click (D-017).
6. Schema changes = a new migration file appended to `MIGRATIONS` in `db.rs`; never edit a released one.
7. Prompt changes = a new versioned prompt/schema file and version constant, not an in-place edit.
8. After changing a type in `models.rs`/`settings.rs`/`secrets.rs`, run the core tests and commit `src/bindings`.
