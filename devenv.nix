{ pkgs, lib, config, inputs, ... }:

{
  # https://devenv.sh/packages/
  packages = [ pkgs.git ];

  # https://devenv.sh/languages/
  # languages.rust.enable = true;
  languages.rust = {
        enable = true;
        channel = "stable";
        version = "1.98.0";
  };

  git-hooks.hooks = {
    clippy = {
        enable = true;
        entry = "cargo clippy --all-targets --all-features -- -D warnings";
    };
  };
}
