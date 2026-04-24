flake:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.zed-rules-sync;
in
{
  options.programs.zed-rules-sync = {
    enable = lib.mkEnableOption "zed-rules-sync";

    package = lib.mkOption {
      type = lib.types.package;
      default = flake.packages.${pkgs.system}.default;
      defaultText = lib.literalExpression "flake.packages.\${pkgs.system}.default";
      description = "The zed-rules-sync package to use.";
    };

    rules = lib.mkOption {
      type = lib.types.path;
      description = "Directory containing .md rule files to sync into Zed.";
    };

    defaultRules = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Whether synced rules should be marked as default (auto-included in every Zed agent thread).";
    };

    prune = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Remove managed rules whose source .md file no longer exists.";
    };

    dbPath = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        Override the path to Zed's prompt store LMDB database. Useful for
        Zed Preview, a custom XDG_CONFIG_HOME, or sandboxed installs. When
        null, the CLI falls back to its default location.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    home.activation.zedRulesSync = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
      run ${cfg.package}/bin/zed-rules-sync ${
        lib.optionalString (cfg.dbPath != null) "--db-path ${cfg.dbPath}"
      } sync ${cfg.rules} \
        ${lib.optionalString cfg.defaultRules "--default"} \
        ${lib.optionalString cfg.prune "--prune"}
    '';
  };
}
