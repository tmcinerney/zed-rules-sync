# zed-rules-sync

Sync markdown rule files into Zed's Rules Library from the command line.

## Why

Zed's [Rules Library](https://zed.dev/docs/assistant/rules-library) is backed by
an LMDB database with no filesystem import path. If you manage AI rules as
markdown files — in a dotfiles repo, a shared team directory, or anywhere on
disk — there's no built-in way to get them into Zed's global Rules Library.

`zed-rules-sync` bridges that gap. Point it at a directory of `.md` files and it
writes them straight into Zed's prompt store, the same LMDB database Zed reads
at startup.

## How It Works

1. Reads every `.md` file in the source directory.
2. Generates a **deterministic UUID** from each filename (UUIDv5 in a
   project-specific namespace), so re-runs update existing entries rather than
   creating duplicates.
3. Writes directly to Zed's LMDB prompt store using the same
   [`heed`](https://github.com/meilisearch/heed) crate Zed uses internally.
4. **Only touches rules it created** — user-created rules are never read,
   modified, or deleted.

## Installation

### Try it

```sh
nix run github:tmcinerney/zed-rules-sync -- list
```

### Flake input

```nix
# flake.nix
{
  inputs.zed-rules-sync.url = "github:tmcinerney/zed-rules-sync";

  outputs = { self, nixpkgs, zed-rules-sync, ... }: {
    # Option A: use the overlay
    nixpkgs.overlays = [ zed-rules-sync.overlays.default ];

    # Option B: reference the package directly
    environment.systemPackages = [
      zed-rules-sync.packages.${system}.default
    ];
  };
}
```

### Home Manager module

```nix
# flake.nix
{
  inputs.zed-rules-sync.url = "github:tmcinerney/zed-rules-sync";

  outputs = { self, nixpkgs, home-manager, zed-rules-sync, ... }: {
    homeConfigurations."user" = home-manager.lib.homeManagerConfiguration {
      modules = [
        zed-rules-sync.homeManagerModules.default
        {
          programs.zed-rules-sync = {
            enable = true;
            rules = ./rules;       # directory of .md files
            defaultRules = false;   # true = auto-include in every agent thread
            prune = true;           # remove managed rules whose source is gone
          };
        }
      ];
    };
  };
}
```

The activation hook runs `zed-rules-sync sync` after `writeBoundary` on every
Home Manager generation switch, so your rules stay in sync with your dotfiles.

## Usage

```
zed-rules-sync <command> [options]
```

### `sync`

Import `.md` files into Zed's Rules Library.

```sh
# Sync all .md files from a directory
zed-rules-sync sync ./rules

# Mark synced rules as default (auto-included in agent threads)
zed-rules-sync sync ./rules --default

# Remove managed rules whose source file no longer exists
zed-rules-sync sync ./rules --prune

# Combine flags
zed-rules-sync sync ./rules --default --prune

# Preview what would happen without writing
zed-rules-sync sync ./rules --dry-run
```

### `list`

List all rules in Zed's Rules Library.

```sh
zed-rules-sync list
```

### `remove`

Remove a managed rule by title.

```sh
zed-rules-sync remove "My Rule Title"
```

## How Rules Map

| Source file | Rule title | UUID |
|---|---|---|
| `rules/code-style.md` | `code-style` | UUIDv5(`namespace`, `"code-style"`) |
| `rules/rust-conventions.md` | `rust-conventions` | UUIDv5(`namespace`, `"rust-conventions"`) |
| `rules/testing.md` | `testing` | UUIDv5(`namespace`, `"testing"`) |

- The **title** is the filename without the `.md` extension.
- The **UUID** is derived deterministically from the title using a fixed
  project-specific UUIDv5 namespace. This means the same filename always
  produces the same UUID across machines and runs.
- The **body** is the full contents of the `.md` file.

## Safety

- **Deterministic UUIDs** prevent duplicates — re-running sync updates existing
  entries in place.
- **Namespace isolation** — the tool tracks which UUIDs it created. User-created
  rules (those made through Zed's UI) are never touched.
- **`--dry-run`** — preview all changes before writing anything.
- **Zed running detection** — warns if Zed is running, since Zed caches rules
  in memory and won't see changes until restarted.
- **Schema version checking** — validates the LMDB database structure before
  writing to prevent data corruption if Zed changes its internal format.

## Keeping Up with Zed

The tool mirrors Zed's `prompt_store` schema. Specifically:

- It reads and writes to the same LMDB databases Zed uses (`bodies`,
  `metadata.v2`, etc.).
- `heed` is pinned to the same version Zed uses to ensure binary compatibility
  of the LMDB file format.

**If Zed changes the schema** (e.g. introduces `metadata.v3` or restructures
the database), the tool will detect the missing or changed databases and
**refuse to write** rather than risk corrupting data. When that happens, an
updated release of `zed-rules-sync` will follow.

Keep an eye on [Zed's releases](https://github.com/zed-industries/zed/releases)
for breaking changes to the prompt store.

## Limitations

- **Restart required** — Zed caches the Rules Library in memory at startup.
  Changes made by `zed-rules-sync` are not visible until Zed is restarted.
- **Default flag is per-rule metadata** — marking a rule as `--default` sets it
  in the LMDB metadata. It won't retroactively apply to already-open agent
  threads.
- **LMDB single-writer** — if Zed is actively writing to the prompt store at
  the exact moment the tool runs, the write may fail. In practice this is rare
  since Zed writes at startup and on explicit user action.

## Contributing

PRs welcome! This project is MIT-licensed.

1. Fork & clone
2. `nix develop` for a shell with all dependencies
3. `cargo build` / `cargo test`
4. Open a PR

## License

[MIT](LICENSE) © 2026 Travers McInerney
