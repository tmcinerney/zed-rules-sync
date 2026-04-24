{
  description = "Sync markdown rule files into Zed's Rules Library";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      ...
    }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;

      mkPackage =
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          craneLib = crane.mkLib pkgs;
        in
        craneLib.buildPackage {
          src = craneLib.cleanCargoSource ./.;

          strictDeps = true;

          # lmdb-master-sys compiles LMDB from bundled C source via the cc crate.
          nativeBuildInputs = with pkgs; [ pkg-config ];

          buildInputs =
            [ ]
            ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin (
              with pkgs.darwin.apple_sdk.frameworks;
              [
                Security
                SystemConfiguration
              ]
            );
        };
    in
    {
      packages = forAllSystems (system: rec {
        zed-rules-sync = mkPackage system;
        default = zed-rules-sync;
      });

      overlays.default = final: _prev: {
        zed-rules-sync = self.packages.${final.system}.default;
      };

      homeManagerModules.default = import ./nix/hm-module.nix self;

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            inputsFrom = [ (mkPackage system) ];
            packages = with pkgs; [
              rust-analyzer
              clippy
            ];
          };
        }
      );
    };
}
