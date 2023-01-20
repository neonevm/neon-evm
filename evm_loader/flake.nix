{
  inputs = {
    cargo2nix.url = "github:cargo2nix/cargo2nix/unstable";
    flake-utils.follows = "cargo2nix/flake-utils";
    nixpkgs.follows = "cargo2nix/nixpkgs";
  };

  outputs = { self, cargo2nix, flake-utils, nixpkgs, ... }:
  flake-utils.lib.eachDefaultSystem (system:
  let
    pkgs = import nixpkgs {
      inherit system;
      overlays = [ cargo2nix.overlays.default ];
    };

    rust-pkgs = pkgs.rustBuilder.makePackageSet {
      rustVersion = "1.66.1";
      packageFun = import ./Cargo.nix;

      extraRustComponents = [
        "rustfmt" "rustc" "clippy" "llvm-tools-preview" "rust-src" "rust-analyzer"
      ];

      packageOverrides = pkgs: pkgs.rustBuilder.overrides.all ++ [
        (pkgs.rustBuilder.rustLib.makeOverride {
          name = "hidapi";
          overrideAttrs = drv: {
            propagatedBuildInputs = (drv.propagatedBuildInputs or []) ++ [
              pkgs.udev
            ];
          };
        })
        (pkgs.rustBuilder.rustLib.makeOverride {
          name = "evm-loader";
          overrideAttrs = drv: {
            NEON_REVISION = "1";
          };
        })
        (pkgs.rustBuilder.rustLib.makeOverride {
          name = "neon-cli";
          overrideAttrs = drv: {
            NEON_REVISION = "1";
          };
        })
      ];
    };

    package = rust-pkgs.workspace.neon-cli {};

    shell = rust-pkgs.workspaceShell {
    };
  in {
    packages.default = package;
    devShells.default = shell;

    devShells.bpf = pkgs.mkShell {
      buildInputs = [ pkgs.rustup ];
      shellHook = ''
        echo "WARNING: You need to install the solana toolchain from within this shell manually to use it"
      '';
    };
  });
}
