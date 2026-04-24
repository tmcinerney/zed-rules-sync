# zed-rules-sync — project notes

Small Rust CLI that syncs `.md` files into Zed's LMDB-backed prompt store.
Distributed as a Nix flake with a Home Manager module. Users pin by git tag.

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
