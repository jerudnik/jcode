{
  description = "Jcode development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      forAllSystems = f:
        nixpkgs.lib.genAttrs systems (system:
          f (import nixpkgs {
            inherit system;
            config.allowUnfree = true;
          }));
    in
    {
      devShells = forAllSystems (pkgs:
        let
          darwinFrameworks = pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.apple-sdk
          ];

          linuxNativeDeps = pkgs.lib.optionals pkgs.stdenv.isLinux (with pkgs; [
            alsa-lib
            fontconfig
            libxkbcommon
            openssl
            wayland
            xorg.libX11
            xorg.libxcb
            xorg.libXcursor
            xorg.libXi
            xorg.libXrandr
          ]);
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo
              clippy
              rustc
              rustfmt
              rust-analyzer

              pkg-config
              openssl
              cmake
              git
              sccache

              python3
              nodejs
            ] ++ darwinFrameworks ++ linuxNativeDeps;

            env = {
              RUST_BACKTRACE = "1";
              CARGO_INCREMENTAL = "1";
            } // pkgs.lib.optionalAttrs pkgs.stdenv.isDarwin {
              # Darwin frameworks are added to packages above; avoid pinning
              # SDKROOT so nixpkgs can choose the current supported SDK.
            } // pkgs.lib.optionalAttrs pkgs.stdenv.isLinux {
              PKG_CONFIG_PATH = pkgs.lib.makeSearchPath "lib/pkgconfig" linuxNativeDeps;
              OPENSSL_NO_VENDOR = "1";
            };

            shellHook = ''
              export CARGO_HOME="''${CARGO_HOME:-$HOME/.cargo}"
              echo "Jcode dev shell: $(rustc --version), $(cargo --version)"
              echo "Try: scripts/dev_cargo.sh build --profile selfdev -p jcode --bin jcode"
            '';
          };
        });

      formatter = forAllSystems (pkgs: pkgs.nixfmt-rfc-style);
    };
}
