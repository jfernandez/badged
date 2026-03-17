self:
{ config, lib, pkgs, ... }:

let
  cfg = config.services.badged;
in {
  options.services.badged = {
    enable = lib.mkEnableOption "badged polkit authentication agent";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.badged;
      defaultText = lib.literalExpression "inputs.badged.packages.${pkgs.stdenv.hostPlatform.system}.badged";
      description = "The badged package to run.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.user.services.badged = {
      Unit = {
        Description = "badged - Polkit Authentication Agent";
        PartOf = [ config.wayland.systemd.target ];
        After = [ config.wayland.systemd.target ];
      };
      Install = {
        WantedBy = [ config.wayland.systemd.target ];
      };
      Service = {
        Type = "simple";
        ExecStart = "${cfg.package}/bin/badged";
        Restart = "on-failure";
        RestartSec = 3;
      };
    };
  };
}
