# Home Manager module for jcode.
#
# Exposed via `inputs.jcode.homeManagerModules.default`. Intentionally thin and
# unopinionated: it installs the package, optionally manages JCODE_HOME, and
# lets users declare their `~/.jcode/config.toml` as freeform Nix attrs (or an
# explicit file) without baking any defaults into the module itself.
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
  configTarget =
    if cfg.home == null then
      ".jcode/config.toml"
    else if lib.hasPrefix "~/" cfg.home then
      "${lib.removePrefix "~/" cfg.home}/config.toml"
    else if lib.hasPrefix "$HOME/" cfg.home then
      "${lib.removePrefix "$HOME/" cfg.home}/config.toml"
    else if lib.hasPrefix "/" cfg.home then
      "__invalid_absolute_jcode_home__/config.toml"
    else
      "${cfg.home}/config.toml";
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
        Declarative `~/.jcode/config.toml` contents, written as TOML. Left
        empty by default so jcode's own defaults apply. Mutually exclusive with
        `configFile`.
      '';
    };

    configFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        Path to a pre-authored `config.toml`. Takes precedence over `settings`.
        Use when you want full control over the file (comments, ordering, etc.).
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
        message = "programs.jcode: when managing config.toml, `home` must be null, home-relative (for example `~/.jcode`), or relative to the Home Manager home directory.";
      }
    ];

    home.packages = [ cfg.package ];

    home.sessionVariables = lib.mkIf (cfg.home != null) {
      JCODE_HOME = cfg.home;
    };

    # jcode reads `~/.jcode/config.toml` (or `$JCODE_HOME/config.toml`). We only
    # manage it when the user opts in via `configFile` or `settings`.
    home.file = lib.mkIf managesConfig {
      ${configTarget} =
        if cfg.configFile != null then
          { source = cfg.configFile; }
        else
          { source = tomlFormat.generate "jcode-config.toml" cfg.settings; };
    };
  };
}
