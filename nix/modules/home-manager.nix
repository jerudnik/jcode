# Home Manager module for jcode.
#
# Exposed via `inputs.jcode.homeManagerModules.default`. Intentionally thin and
# unopinionated: it installs the package, optionally manages JCODE_HOME, and
# lets users declare their `~/.jcode/config.nix.toml` policy as freeform Nix attrs
# (or an explicit file) without baking any defaults into the module itself.
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.jcode;
  tomlFormat = pkgs.formats.toml { };
  managesConfig = cfg.configFile != null || cfg.settings != { };
  configTargetIsValid = cfg.home == null || !(lib.hasPrefix "/" cfg.home);
  configFileName = if cfg.manageConfigToml then "config.toml" else "config.nix.toml";
  configTarget =
    if cfg.home == null then
      ".jcode/${configFileName}"
    else if lib.hasPrefix "~/" cfg.home then
      "${lib.removePrefix "~/" cfg.home}/${configFileName}"
    else if lib.hasPrefix "$HOME/" cfg.home then
      "${lib.removePrefix "$HOME/" cfg.home}/${configFileName}"
    else if lib.hasPrefix "/" cfg.home then
      "__invalid_absolute_jcode_home__/${configFileName}"
    else
      "${cfg.home}/${configFileName}";
in
{
  options.programs.jcode = {
    enable = lib.mkEnableOption "jcode coding agent";

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.jcode;
      defaultText = lib.literalExpression "pkgs.jcode";
      description = ''
        The jcode package to install. Defaults to `pkgs.jcode`, which is
        provided when the flake's `overlays.default` is applied.
      '';
    };

    home = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = null;
      example = "~/.jcode";
      description = ''
        Value for the `JCODE_HOME` environment variable. When null, jcode uses
        its default (`~/.jcode`). Set this to relocate jcode's state directory.
        If this module also manages `config.nix.toml` or `config.toml` via
        `settings` or `configFile`, the value must be null, home-relative (for
        example `~/.local/state/jcode`), or relative to the Home Manager home
        directory; absolute paths are allowed
        only when this module is not managing the config file.
      '';
    };

    settings = lib.mkOption {
      inherit (tomlFormat) type;
      default = { };
      example = lib.literalExpression ''
        {
          display.diff_mode = "inline";
          keybindings.scroll_up = "ctrl+k";
        }
      '';
      description = ''
        Declarative `~/.jcode/config.nix.toml` policy contents, written as TOML.
        Policy keys are pinned: jcode reads them but writes mutable runtime
        preferences to `config.toml`. Left empty by default so jcode's own
        defaults apply. Mutually exclusive with `configFile`.
      '';
    };

    configFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        Path to a pre-authored TOML policy file. Takes precedence over `settings`
        and is installed as `config.nix.toml` unless `manageConfigToml` is true.
        Use when you want full control over the file (comments, ordering, etc.).
      '';
    };

    manageConfigToml = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Manage `$JCODE_HOME/config.toml` instead of the default policy file
        `$JCODE_HOME/config.nix.toml`. This restores the old full-ownership
        behavior, but it can break jcode runtime config writes because Home
        Manager normally installs a read-only store symlink at `config.toml`.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = !(cfg.configFile != null && cfg.settings != { });
        message = "programs.jcode: set either `settings` or `configFile`, not both.";
      }
      {
        assertion = !managesConfig || configTargetIsValid;
        message = "programs.jcode: when managing a config file, `home` must be null, home-relative (for example `~/.jcode`), or relative to the Home Manager home directory.";
      }
    ];

    warnings = lib.optional (managesConfig && cfg.manageConfigToml) ''
      programs.jcode.manageConfigToml = true restores legacy full ownership of
      config.toml. jcode mutates config.toml at runtime, so a read-only Home
      Manager symlink can make model switches, trust decisions, and other saves fail.
      Prefer the default config.nix.toml policy layer unless you intentionally want
      to own the whole durable config file.
    '';

    home.packages = [ cfg.package ];

    home.sessionVariables = lib.mkIf (cfg.home != null) {
      JCODE_HOME = cfg.home;
    };

    # jcode reads `config.nix.toml` as a pinned policy layer by default. The mutable
    # durable `config.toml` remains jcode-owned unless `manageConfigToml` is set.
    home.file = lib.mkIf managesConfig {
      ${configTarget} =
        if cfg.configFile != null then
          { source = cfg.configFile; }
        else
          { source = tomlFormat.generate "jcode-config.toml" cfg.settings; };
    };
  };
}
