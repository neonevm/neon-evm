{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    rust-overlay.inputs.flake-utils.follows = "flake-utils";

    cargo2nix.url = "github:cargo2nix/cargo2nix/unstable";
    cargo2nix.inputs.nixpkgs.follows = "nixpkgs";
    cargo2nix.inputs.flake-utils.follows = "flake-utils";
    cargo2nix.inputs.rust-overlay.follows = "rust-overlay";
  };

  outputs = { self, cargo2nix, flake-utils, nixpkgs, ... }:
  flake-utils.lib.eachDefaultSystem (system:
  let
    pkgs = import nixpkgs {
      inherit system;
      overlays = [ cargo2nix.overlays.default ];
    };

    rust-pkgs = pkgs.rustBuilder.makePackageSet {
      rustVersion = "1.67.0";
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
