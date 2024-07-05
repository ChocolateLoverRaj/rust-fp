{
  description = "A devShell example";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }: flake-utils.lib.eachDefaultSystem
    (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
      {
        devShells.default = with pkgs; mkShell {
          buildInputs = [
            rustPlatform.bindgenHook
            pam
            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" ];
            })
          ];
        };
      }
    ) // {
    nixosModules.default = { lib, pkgs, ... }: {
      config =
        let
          rust-fp-dbus-interface-config = (pkgs.stdenv.mkDerivation rec {
            name = "rust-fp-pam";
            src = ./dbus-interface;
            installPhase = ''
              mkdir -p $out/share/dbus-1/system.d
              cp $src/org.rust_fp.RustFp.conf $out/share/dbus-1/system.d
              echo Cros FP Pam output at $out
            '';
          });
          _cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              "crosec-0.1.0" = "sha256-/G0/ClCZUdBv0a8fl/UUsXVCHD2V4Ts97oyQvfi23hE=";
            };
          };
          rust-fp-dbus-interface = with pkgs; with builtins; rustPlatform.buildRustPackage
            (
              let
                cargoToml = (fromTOML (readFile ./dbus-interface/Cargo.toml));
              in
              {
                pname = cargoToml.package.name;
                version = cargoToml.package.version;
                cargoLock = _cargoLock;
                src = ./.;
                buildAndTestSubdir = "dbus-interface";
                nativeBuildInputs = [
                  rustPlatform.bindgenHook
                  rustPlatform.cargoBuildHook
                ];
              }
            );
          rust-fp-cli = with pkgs; with builtins; rustPlatform.buildRustPackage (
            let
              cargoToml = (fromTOML (readFile ./cli/Cargo.toml));
            in
            {
              pname = cargoToml.package.name;
              version = cargoToml.package.version;
              cargoLock = _cargoLock;
              src = ./.;
              buildAndTestSubdir = "cli";
              nativeBuildInputs = [
                rustPlatform.bindgenHook
                rustPlatform.cargoBuildHook
              ];
            }
          );
          rust-fp-pam-module = with pkgs;  with builtins;rustPlatform.buildRustPackage (
            let
              cargoToml = (fromTOML (readFile ./pam-module/Cargo.toml));
            in
            {
              pname = cargoToml.package.name;
              version = cargoToml.package.version;
              cargoLock = _cargoLock;
              src = ./.;
              buildAndTestSubdir = "pam-module";
              nativeBuildInputs = [
                rustPlatform.bindgenHook
                rustPlatform.cargoBuildHook
              ];
              buildInputs = [
                pam
              ];
            }
          );
        in
        {
          systemd.services.rust-fp-dbus-interface = {
            enable = true;
            description = "Gives normal user access to enrolling and matching fingerprints";
            serviceConfig = {
              Type = "exec";
              ExecStart = "${rust-fp-dbus-interface}/bin/rust-fp-dbus-interface";
            };
            wantedBy = [ "multi-user.target" ];
          };

          # Example: https://github.com/NixOS/nixpkgs/issues/239770#issuecomment-1608589113
          security.pam.services.kde-fingerprint.text = ''
            auth    sufficient    ${rust-fp-pam-module}/lib/librust_fp_pam_module.so
            account sufficient    ${rust-fp-pam-module}/lib/librust_fp_pam_module.so
          '';

          environment.systemPackages = [
            rust-fp-dbus-interface-config
            rust-fp-cli
          ];
        };
    };
  };
}
