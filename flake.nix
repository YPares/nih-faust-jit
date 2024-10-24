{
  description = "Build a cargo project without extra checks";

  nixConfig = {
    extra-substituters = [ "https://cache.garnix.io" ];
    extra-trusted-public-keys =
      [ "cache.garnix.io:CTFPyKSLcx5RMJKfLo5EEPUObbA78b0YQ2DTCJXqr9g=" ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane.url = "github:ipetkov/crane";

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, crane, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        craneLib = crane.mkLib pkgs;

        env = {
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
          FAUST_LIB = "faust";
          DSP_LIBS_PATH = "${pkgs.faust}/share/faust";
        };

        alsa-overriden = with pkgs;
          alsa-lib-with-plugins.override {
            plugins = symlinkJoin {
              name = "alsa-plugins";
              paths = [ alsa-plugins pipewire ];
            };
          };

        # Note: changes here will rebuild all dependency crates
        commonArgs = with pkgs; {
          src =
            ./.; # Cannot use craneLib.cleanCargoSource because of faust_jit/c_src
          strictDeps = true;

          nativeBuildInputs = [ pkg-config ];

          buildInputs = [ alsa-overriden libGL xorg.libX11 libjack2 faust ]
            ++ lib.optionals stdenv.isDarwin [ libiconv ];

          inherit env;
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # faust_jit = craneLib.buildPackage (commonArgs // {
        #   inherit cargoArtifacts;
        #   cargoToml = ./faust_jit/Cargo.toml;
        #   cargoExtraArgs = "-p faust_jit";
        # });

        # faust_jit_egui = craneLib.buildPackage (commonArgs // {
        #   inherit cargoArtifacts;
        #   cargoToml = ./faust_jit_egui/Cargo.toml;
        #   cargoExtraArgs = "-p faust_jit_egui";
        # });

        nih_faust_jit = craneLib.mkCargoDerivation (commonArgs // {
          inherit cargoArtifacts;
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
      in {
        packages = {
          default = nih_faust_jit;
          inherit # faust_jit faust_jit_egui
            nih_faust_jit;
        };

        checks = {
          inherit # faust_jit faust_jit_egui
            nih_faust_jit;
        };

        apps = rec {
          default = nih_faust_jit_standalone;
          nih_faust_jit_standalone = flake-utils.lib.mkApp {
            drv = nih_faust_jit;
            name = "nih_faust_jit_standalone";
          };
        };

        devShells.default = craneLib.devShell {
          # Inherit inputs from checks.
          checks = self.checks.${system};
          inherit env;
        };
      });
}
