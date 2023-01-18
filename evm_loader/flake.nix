{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, flake-utils, naersk, nixpkgs, rust-overlay, ... }:
  flake-utils.lib.eachDefaultSystem (system:
  let
    pkgs = import nixpkgs {
      inherit system;
      overlays = [ (import rust-overlay) ];
    };

    rust-toolchain = pkgs.rust-bin.stable.latest.default.override {
      extensions = [
        "rustfmt" "rustc" "clippy" "llvm-tools-preview" "rust-src" "rust-analyzer"
      ];
    };

    naersk' = pkgs.callPackage naersk {
      cargo = rust-toolchain;
      rustc = rust-toolchain;
    };

    package = naersk'.buildPackage {
      src = ./.;

      nativeBuildInputs = [
        pkgs.pkg-config
      ];

      buildInputs = [
        pkgs.openssl pkgs.udev
      ];
    };

    shell = pkgs.mkShell {
      inputsFrom = [ package ];
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
