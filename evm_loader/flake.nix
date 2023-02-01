{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    rust-overlay.inputs.flake-utils.follows = "flake-utils";

    naersk.url = "github:nix-community/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, naersk, flake-utils, nixpkgs, rust-overlay, ... }:
  flake-utils.lib.eachDefaultSystem (system:
  let
    pkgs = import nixpkgs {
      inherit system;
      overlays = [ (import rust-overlay) ];
    };

    # To update the Rust toolchain, just run `nix flake update` to update the flake.lock file.
    # If there's an update on the toolchain, the rust-overlay input should be updated.
    toolchain = pkgs.rust-bin.stable.latest.default.override {
      extensions = [
        "rustfmt" "rustc" "clippy" "llvm-tools-preview" "rust-src" "rust-analyzer"
      ];
    };

    naersk' = pkgs.callPackage naersk {
      cargo = toolchain;
      rustc = toolchain;
    };

    package = naersk'.buildPackage {
      src = ./.;
      buildInputs = [
        pkgs.udev
        pkgs.openssl
      ];

      NEON_REVISION = "1";
    };

    shell = pkgs.mkShell {
      inputsFrom = [ package ];

      NEON_REVISION = "1";
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
