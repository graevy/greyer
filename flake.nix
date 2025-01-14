# never really got this to work, but the gist is that you have to use bindgenHook to get ffmpeg to build at all
{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, naersk }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };
        ffmpeg = pkgs.ffmpeg-full;
      in
      {
        defaultPackage = naersk-lib.buildPackage {
          src = ./.;
          nativeBuildInputs = [ ffmpeg pkgs.rustPlatform.bindgenHook ];
        };
        devShell = with pkgs; mkShell {
          buildInputs = [ cargo rustc rustfmt pre-commit rustPackages.clippy ffmpeg pkg-config ];
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
	  PKG_CONFIG_PATH = "${ffmpeg.dev}/lib/pkgconfig:${pkgs.pkg-config}/lib/pkgconfig:$PKG_CONFIG_PATH";
          PKG_CONFIG = "${pkgs.pkg-config}/bin/pkg-config";
        };
      }
    );
}

