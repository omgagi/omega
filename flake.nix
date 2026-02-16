{
  description = "Omega — personal AI agent infrastructure";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = fn: nixpkgs.lib.genAttrs systems (system:
        fn {
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          inherit system;
        });
    in
    {
      devShells = forAllSystems ({ pkgs, system }: {
        default = pkgs.mkShell {
          buildInputs = with pkgs; [
            # Rust toolchain (nightly, required by whatsapp-rust's portable_simd).
            (rust-bin.nightly."2025-12-01".default.override {
              extensions = [ "rust-src" "rust-analyzer" ];
            })

            # Build deps.
            pkg-config
            sqlite

            # Runtime: Claude Code CLI must be on PATH (installed separately).
          ];

          env = {
            # Point libsqlite3-sys to the nix-provided sqlite.
            SQLITE3_LIB_DIR = "${pkgs.sqlite.out}/lib";
          };

          shellHook = ''
            echo "omega dev shell — $(rustc --version)"
          '';
        };
      });
    };
}
