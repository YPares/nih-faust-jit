{
  description = "nih_faust_jit CLAP, VST3 & standalone plugin";

  nixConfig = {
    extra-substituters = [ "https://cache.garnix.io" ];
    extra-trusted-public-keys =
      [ "cache.garnix.io:CTFPyKSLcx5RMJKfLo5EEPUObbA78b0YQ2DTCJXqr9g=" ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, fenix, crane, ... }:
    let
      forEachSystem = fn: with nixpkgs.lib;
        zipAttrsWith (_: mergeAttrsList) (map fn systems.flakeExposed);
    in
    forEachSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        toolchain = fenix.packages.${system}.fromToolchainFile {
          file = ./rust-toolchain.toml;
          sha256 = "sha256-yMuSb5eQPO/bHv+Bcf/US8LVMbf/G/0MSfiPwBhiPpk=";
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;

        env = {
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          FAUST_LIB = "faust";
          DSP_LIBS_PATH = "${pkgs.faust}/share/faust";
        };

        # Note: changes here will rebuild all dependency crates
        commonArgs = with pkgs; {
          src =
            ./.; # Cannot use craneLib.cleanCargoSource because of faust_jit/c_src
          strictDeps = true;

          nativeBuildInputs = [ pkg-config ];

          buildInputs = [ alsa-lib libGL xorg.libX11 libjack2 faust ]
            ++ lib.optionals stdenv.isDarwin [ libiconv ];

          inherit env;
        };

        depsArtifacts = craneLib.buildDepsOnly (commonArgs // {
          pname = "nih_faust_jit-deps";
          version = "0.1.0";
        });

        faust_jit = craneLib.buildPackage (commonArgs // {
          cargoArtifacts = depsArtifacts;
          cargoToml = ./faust_jit/Cargo.toml;
          cargoExtraArgs = "-p faust_jit";
          doInstallCargoArtifacts = true;
        });

        faust_jit_egui = craneLib.buildPackage (commonArgs // {
          cargoArtifacts = faust_jit;
          cargoToml = ./faust_jit_egui/Cargo.toml;
          cargoExtraArgs = "-p faust_jit_egui";
          doInstallCargoArtifacts = true;
        });

        nih_faust_jit = craneLib.mkCargoDerivation (commonArgs // {
          cargoArtifacts = faust_jit_egui;
          cargoToml = ./nih_faust_jit/Cargo.toml;
          buildPhase = ''
            cargo build --release
            cargo xtask bundle nih_faust_jit --release
          '';
          buildPhaseCargoCommand = "";
          installPhase = ''
            mkdir -p $out/bin
            cp target/release/nih_faust_jit_standalone $out/bin
            cp -R target/bundled $out/plugin
          '';
        });

        # Standalone exe wrapped to use Pipewire for ALSA or Jack (for Ubuntu notably)
        nih_faust_jit_pipewire = pkgs.writeShellApplication {
            name = "nih_faust_jit_pipewire";
            text = ''
                export ALSA_PLUGIN_DIR=${pkgs.pipewire}/lib/alsa-lib
                ${pkgs.pipewire.jack}/bin/pw-jack ${nih_faust_jit}/bin/nih_faust_jit_standalone "$@"
            '';
        };
      in
      {
        packages.${system} = {
          default = nih_faust_jit;
          inherit faust_jit faust_jit_egui nih_faust_jit nih_faust_jit_pipewire toolchain;
        };

        checks.${system} = {
          inherit faust_jit faust_jit_egui nih_faust_jit;
        };

        apps.${system} = {
          default = self.apps.${system}.nih_faust_jit_standalone;
          nih_faust_jit_standalone = {
            type = "app";
            program = "${nih_faust_jit}/bin/nih_faust_jit_standalone";
          };
          nih_faust_jit_pipewire = {
            type = "app";
            program = "${nih_faust_jit_pipewire}/bin/nih_faust_jit_pipewire";
          };
          pw-jack = {
            type = "app";
            program = "${pkgs.pipewire.jack}/bin/pw-jack";
          };
        };

        devShells.${system}.default = craneLib.devShell {
          # Inherit inputs from checks.
          checks = self.checks.${system};
          inherit env;
        };
      });
}
