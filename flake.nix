{
  description = "AI shell-command generator for zsh and bash";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      lib = nixpkgs.lib;
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      releaseOwner = "abdulrahman1s";
      releaseRepo = "qsh";
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      prebuiltTargets = {
        x86_64-linux = "x86_64-unknown-linux-gnu";
        x86_64-darwin = "x86_64-apple-darwin";
        aarch64-darwin = "aarch64-apple-darwin";
      };
      forAllSystems = lib.genAttrs systems;
      mkPkgs = system: import nixpkgs { inherit system; };
      source = lib.cleanSourceWith {
        src = ./.;
        filter =
          path: type:
          let
            name = baseNameOf path;
          in
          lib.cleanSourceFilter path type
          && !(type == "directory" && name == "target")
          && !(name == "result" || lib.hasPrefix "result-" name);
      };
      mkPackage =
        pkgs:
        pkgs.rustPlatform.buildRustPackage {
          pname = cargoToml.package.name;
          version = cargoToml.package.version;

          src = source;
          cargoLock.lockFile = ./Cargo.lock;

          meta = {
            description = "AI shell-command generator for zsh and bash";
            mainProgram = cargoToml.package.name;
            platforms = lib.platforms.unix;
          };
        };
      mkPrebuiltPackage =
        pkgs:
        {
          version ? cargoToml.package.version,
          hash,
          owner ? releaseOwner,
          repo ? releaseRepo,
          target ?
            prebuiltTargets.${pkgs.stdenv.hostPlatform.system}
              or (throw "No qsh prebuilt target for ${pkgs.stdenv.hostPlatform.system}"),
          url ? null,
        }:
        let
          system = pkgs.stdenv.hostPlatform.system;
          releaseVersion = lib.removePrefix "v" version;
          releaseTag = "v${releaseVersion}";
          archiveUrl =
            if url != null then
              url
            else if target == null then
              throw "No qsh prebuilt target was provided for ${system}"
            else
              "https://github.com/${owner}/${repo}/releases/download/${releaseTag}/${cargoToml.package.name}-${releaseTag}-${target}.tar.gz";
        in
        pkgs.stdenvNoCC.mkDerivation {
          pname = "${cargoToml.package.name}-prebuilt";
          version = releaseVersion;

          src = pkgs.fetchurl {
            url = archiveUrl;
            hash = if hash == "" then lib.fakeHash else hash;
          };

          sourceRoot = ".";
          dontConfigure = true;
          dontBuild = true;

          installPhase = ''
            runHook preInstall
            install -Dm0755 ${cargoToml.package.name} "$out/bin/${cargoToml.package.name}"
            runHook postInstall
          '';

          meta = {
            description = "Prebuilt qsh binary from GitHub Releases";
            mainProgram = cargoToml.package.name;
            platforms = builtins.attrNames prebuiltTargets;
          };
        };
    in
    {
      lib.mkPrebuiltPackage = mkPrebuiltPackage;

      packages = forAllSystems (
        system:
        let
          pkgs = mkPkgs system;
        in
        rec {
          qsh = mkPackage pkgs;
          default = qsh;
        }
      );

      apps = forAllSystems (
        system:
        let
          app = {
            type = "app";
            program = "${self.packages.${system}.default}/bin/qsh";
            meta.description = "Run qsh";
          };
        in
        {
          qsh = app;
          default = app;
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = mkPkgs system;
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo
              clippy
              rust-analyzer
              rustc
              rustfmt
              stdenv.cc
              pkg-config
              mold
            ];
          };
        }
      );

      overlays.default =
        final: _prev:
        let
          system = final.stdenv.hostPlatform.system;
        in
        {
          qsh = self.packages.${system}.default;
        };

      nixosModules.default =
        {
          config,
          lib,
          pkgs,
          ...
        }:
        let
          cfg = config.programs.qsh;
          system = pkgs.stdenv.hostPlatform.system;
          prebuiltPackage = mkPrebuiltPackage pkgs {
            inherit (cfg.prebuilt)
              hash
              owner
              repo
              target
              url
              version
              ;
          };
          package = if cfg.prebuilt.enable then prebuiltPackage else cfg.package;
        in
        {
          options.programs.qsh = {
            enable = lib.mkEnableOption "qsh shell-command generator";

            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${system}.default;
              defaultText = "self.packages.<system>.default";
              description = "The qsh package to install.";
            };

            prebuilt = {
              enable = lib.mkEnableOption "installing qsh from a prebuilt GitHub Release binary instead of building from source";

              version = lib.mkOption {
                type = lib.types.str;
                default = cargoToml.package.version;
                defaultText = "the version in Cargo.toml";
                description = "Release version to fetch, with or without the leading v.";
              };

              hash = lib.mkOption {
                type = lib.types.str;
                default = "";
                example = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
                description = "SRI hash for the release tarball. This is required when prebuilt.enable is true.";
              };

              owner = lib.mkOption {
                type = lib.types.str;
                default = releaseOwner;
                description = "GitHub owner or organization that hosts the release assets.";
              };

              repo = lib.mkOption {
                type = lib.types.str;
                default = releaseRepo;
                description = "GitHub repository that hosts the release assets.";
              };

              target = lib.mkOption {
                type = lib.types.nullOr lib.types.str;
                default = prebuiltTargets.${system} or null;
                defaultText = "the GitHub release target for the host platform";
                description = "Release artifact target triple to fetch.";
              };

              url = lib.mkOption {
                type = lib.types.nullOr lib.types.str;
                default = null;
                example = "https://github.com/abdulrahman1s/qsh/releases/download/v0.2.0/qsh-v0.2.0-x86_64-unknown-linux-gnu.tar.gz";
                description = "Full release tarball URL. When set, owner, repo, version, and target are only used for package metadata.";
              };
            };

            enableZshIntegration = lib.mkOption {
              type = lib.types.bool;
              default = false;
              description = "Add qsh zsh integration to interactive shells.";
            };

            enableBashIntegration = lib.mkOption {
              type = lib.types.bool;
              default = false;
              description = "Add qsh bash integration to interactive shells.";
            };
          };

          config = lib.mkIf cfg.enable {
            assertions = [
              {
                assertion = !cfg.prebuilt.enable || cfg.prebuilt.hash != "";
                message = "programs.qsh.prebuilt.hash must be set when programs.qsh.prebuilt.enable is true.";
              }
              {
                assertion = !cfg.prebuilt.enable || cfg.prebuilt.url != null || cfg.prebuilt.target != null;
                message = "No qsh prebuilt target is known for ${system}; set programs.qsh.prebuilt.target or programs.qsh.prebuilt.url.";
              }
            ];

            environment.systemPackages = [ package ];

            programs.zsh.interactiveShellInit = lib.mkIf cfg.enableZshIntegration (lib.mkAfter ''
              eval "$(${lib.getExe package} init zsh)"
            '');

            programs.bash.interactiveShellInit = lib.mkIf cfg.enableBashIntegration (lib.mkAfter ''
              eval "$(${lib.getExe package} init bash)"
            '');
          };
        };
    };
}
