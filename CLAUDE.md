# zed-rules-sync — project notes

Small Rust CLI that syncs `.md` files into Zed's LMDB-backed prompt store.
Distributed as a Nix flake with a Home Manager module. Users pin by git tag.

## Architecture

- `src/main.rs` — CLI surface (clap). Delegates to `db` + `types`.
- `src/db.rs` — `RulesDb` wrapping heed/LMDB. Opens Zed's `metadata.v2`
  and `bodies.v2` databases.
- `src/types.rs` — serde shapes, namespace/UUID derivation, and
  `is_managed()`. See Invariants.
- `nix/hm-module.nix` — Home Manager module. Activation runs `sync`
  after `writeBoundary` on every HM generation switch.
- `flake.nix` — package (crane), overlay, HM module, devShell.

The flake build uses `craneLib.cleanCargoSource ./.`, so anything
outside cargo-tracked sources (this file, `README.md`, `nix/`,
workflows) is excluded from the built derivation. That's why
docs-only changes can ship on `main` without cutting a tag — the
package output is byte-identical.

## Toolchain

Two Rust toolchains are in play intentionally:

- `devenv shell` uses `rust-overlay`'s stable channel (newer, for
  local ergonomics).
- `nix develop` and `nix build` use nixpkgs' pinned rustc (older,
  what CI runs and what end users build against).

If CI shows a version different from your local, that's expected.
The flake is the source of truth for consumer-visible builds.

## Dev workflow

- `devenv shell` — primary dev shell. Provides cargo, clippy, rustfmt,
  typos, nixfmt-rfc-style, actionlint.
- `nix develop` — alternative shell via flake.nix's devShells.default.
  What CI uses.
- Full local verification:
  `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && nix build .#default`
- Pre-commit hooks are auto-installed by devenv and enforced on
  `git commit`. Don't bypass with `--no-verify`.

## Release discipline

Users discover changes through git tags. A tag is the only reliable signal
that something changed. So:

- Every merge that affects observable behavior — CLI flags, module options,
  LMDB schema, output format, exit codes, activation-script shape — must be
  followed by a version bump in `Cargo.toml`, a tag on that commit, and a
  GitHub release with notes.
- Pure-internal changes (CI tooling, dev deps, comments) may skip the tag.
  When in doubt, tag it. Patch bumps are free.
- Do not batch "polish" as untagged commits past the latest tag. The result
  is that `nix run @vX.Y.Z` and `nix run` unpinned report the same version
  string but ship different behavior.
- Breaking changes in `0.x` still get an explicit call-out in release notes
  with a migration snippet. Type changes on HM module options, schema
  migrations, and CLI flag renames all qualify.

## Branch protection on `main`

- PR required + both CI contexts (`build-test (ubuntu-latest)`,
  `build-test (macos-latest)`) required to pass before merge.
- Admin bypass is allowed (`enforce_admins: false`). Direct pushes for
  release prep (version bumps, tags) are intentional — don't route them
  through a PR unless there's a reason to.
- Auto-delete branches on merge is on. Auto-merge is on.

## Invariants to protect

- `NAMESPACE` in `src/types.rs`. Changing these bytes detaches every
  previously-synced rule on every user's machine. The
  `namespace_bytes_are_frozen` test pins the bytes; the
  `prompt_id_for_filename_has_golden_value` test pins a known UUID. If
  either test ever needs updating, that's a major breaking change and
  needs to be signalled accordingly.
- `is_managed()` in `src/types.rs`. It's the load-bearing gate that keeps
  `sync --prune` and `remove --managed` from touching user-created rules.
  Destructive ops must stay behind it.
- The Zed LMDB schema names (`metadata.v2`, `bodies.v2`) in `src/db.rs`.
  They track Zed upstream — if Zed moves to `v3`, the tool's job is to
  detect the change and refuse to write, not to migrate automatically.
- The `heed` major version in `Cargo.toml`. It's pinned to match the
  version Zed uses so the LMDB on-disk format stays binary-compatible
  with Zed's reader. Don't `cargo update` heed to a new major without
  verifying Zed's current version and testing against a real prompt
  DB. A silent bump here could corrupt user data.
