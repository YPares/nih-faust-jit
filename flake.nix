{
  description = "Build a cargo project without extra checks";

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

        # Note: changes here will rebuild all dependency crates
        commonArgs = with pkgs; {
          src = craneLib.cleanCargoSource ./.;
          strictDeps = true;

          nativeBuildInputs = [ pkg-config ];

          buildInputs = [ alsa-lib libGL xorg.libX11 libjack2 faust ]
            ++ lib.optionals stdenv.isDarwin [ libiconv ];

          env = {
            LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
            FAUST_LIB = "faust";
          };
        };

        individualCrateArgs = crateSrc:
          commonArgs // {
            cargoArtifacts = craneLib.buildDepsOnly commonArgs;
            inherit (craneLib.crateNameFromCargoToml {
              cargoToml = "${crateSrc}/Cargo.toml";
            })
              version;
          };

        faust_jit = craneLib.buildPackage (individualCrateArgs ./faust_jit // {
          src = ./.;
          cargoExtraArgs = "-p faust_jit";
        });

        faust_jit_egui = craneLib.buildPackage
          (individualCrateArgs ./faust_jit_egui // {
            src = ./.;
            cargoExtraArgs = "-p faust_jit_egui";
          });
        
        nih_faust_jit = craneLib.buildPackage
          (individualCrateArgs ./nih_faust_jit // {
            src = ./.;
            cargoExtraArgs = "-p nih_faust_jit";
          });
      in {
        packages = {
          inherit
            faust_jit faust_jit_egui nih_faust_jit;
        };

        checks = { inherit faust_jit; };

        apps.default = flake-utils.lib.mkApp {
          drv = nih_faust_jit;
        };

        devShells.default = craneLib.devShell {
          # Inherit inputs from checks.
          checks = self.checks.${system};
        };
      });
}
