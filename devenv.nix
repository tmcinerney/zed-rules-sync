{ pkgs, ... }:
{
  languages.rust = {
    enable = true;
    channel = "stable";
  };

  packages = with pkgs; [
    pkg-config
    clippy
    rust-analyzer
  ];

  git-hooks.hooks = {
    clippy.enable = true;
    rustfmt.enable = true;
  };
}
