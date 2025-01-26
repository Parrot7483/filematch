{
  description = "A Rust-based tool to compare files between two directories by hash";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
      };
    in
    {
      packages = {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "filematch";
          version = "0.1.1";
          description = "Compares files between two directories by hash";
          src = ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          license = pkgs.lib.licenses.gpl3Plus.fullName;
        };
      };

      devShell = pkgs.mkShell {
        nativeBuildInputs = [
          pkgs.rustc
          pkgs.cargo
          pkgs.rustfmt
          pkgs.clippy
          pkgs.cargo-bloat
          pkgs.cargo-audit
          pkgs.cargo-flamegraph
        ];
      };

      # App output to run benchmarks using `nix run .#bench`
      apps.bench = let
        benchScript = pkgs.writeShellScriptBin "cargo-bench-wrapper" ''
          # Ensure cargo, rustc, and gcc are in PATH.
          # Explicitly set the C compiler.
          # Use a temporary writable directory for cargo's target.
          export PATH="${pkgs.cargo}/bin:${pkgs.rustc}/bin:${pkgs.gcc}/bin:$PATH"
          export CARGO_TARGET_DIR=$(mktemp -d -t cargo-target-XXXXXX)
          cd ${toString ./.}
          cargo bench --bench benchmark
        '';
        benchEnv = pkgs.buildEnv {
          name = "bench-env";
          paths = [ benchScript pkgs.rustc pkgs.cargo pkgs.gcc ];
        };
      in {
        type = "app";
        program = "${benchEnv}/bin/cargo-bench-wrapper";
      };
    });
}

