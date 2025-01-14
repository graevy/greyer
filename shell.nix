# shell.nix
{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = [
    pkgs.ffmpeg
    pkgs.pkg-config
    pkgs.openssl
  ];

  shellHook = ''
  '';
}
