{
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    nixos-generators = {
      url = "github:nix-community/nixos-generators";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = { nixpkgs, flake-utils, nixos-generators, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        fetchaur = { config, pkgs, ... }: {
          systemd = {
            services.fetchaur = rec {
              path = [ pkgs.curl ];
              script = ''
                set -x
                mkdir -p $OUTPUT_DIR
                if [ -z "$URL" ]; then
                  URL="$(curl -s -H 'Metadata-Flavor: Google' 'http://metadata.google.internal/computeMetadata/v1/instance/attributes/url')"
                fi
                curl $URL \
                    --location \
                    --etag-compare /tmp/etag.txt \
                    --etag-save /tmp/etag.txt \
                    --output $OUTPUT_DIR/$(date -u "+%Y-%m-%dT%H:%M").tar.gz
              '';
              serviceConfig = {
                Type = "oneshot";
                EnvironmentFile = "/etc/fetchaur.conf";
              };
            };
            timers.fetchaur = {
              wantedBy = [ "timers.target" ];
              timerConfig = {
                OnBootSec = "1min";
                OnUnitActiveSec = "5min";
                AccuracySec = "1s";
              };
            };
          };

          services.openssh.enable = true;
          users.users.root.openssh.authorizedKeys.keys = [
            "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBA+fsi2MONdZ65XIrD+e5EYfPqcZrG4Fd0E4VMz9YHQ zjn@zjn-work"
          ];
        };
        debug = { config, pkgs, ... }: {
          users.users.root.password = "password";
          environment.etc."fetchaur.conf" = {
            text = ''
              URL=https://example.com/index.html
              OUTPUT_DIR=/tmp
            '';
            mode = "0440";
          };
        };
        gceconf = { config, pkgs, ... }: {
          environment.systemPackages = [ pkgs.google-compute-engine ];
          # TODO: format disk if needed!
          fileSystems."/metadata" = {
            device = "/dev/disk/by-id/google-metadata";
            fsType = "ext4";
          };
          systemd.services.automount = {
            description = "Set up directories if they don't exist.";
            wantedBy = [ "multi-user.target" ];
            path = [ "/run/wrappers" pkgs.file pkgs.coreutils pkgs.e2fsprogs ];
            script = ''
              set -ux
              file -sL /dev/disk/by-id/google-metadata | grep ext4 || mkfs.ext4 /dev/disk/by-id/google-metadata
              mount /dev/disk/by-id/google-metadata
            '';
            serviceConfig = { Type = "oneshot"; };
          };
          environment.etc."fetchaur.conf" = {
            # TODO: main thing is to grab the following via cron every 5 minutes:
            #   https://aur.archlinux.org/packages-meta-ext-v1.json.gz
            # It's about 8.2MB, doing this every 5 minutes for a week is worst-case
            #   ((8.2 × megabyte) / (5 × minute)) × (1 × week) = 16.5312 GB
            # TODO: update issue before changing to packages-meta-ext-v1.json.gz
            text = ''
              OUTPUT_DIR=/metadata
            '';
            mode = "0440";
          };
        };
        pkgs = import nixpkgs { inherit system; };
      in {
        packages = rec {
          gce = nixos-generators.nixosGenerate {
            pkgs = nixpkgs.legacyPackages.x86_64-linux;
            format = "gce";
            modules = [ fetchaur gceconf ];
          };
          qcow = nixos-generators.nixosGenerate {
            pkgs = nixpkgs.legacyPackages.x86_64-linux;
            format = "qcow";
            modules = [ fetchaur debug ];
          };
          default = gce;
        };
        devShells.default =
          pkgs.mkShell { buildInputs = [ pkgs.qemu pkgs.terraform pkgs.jq ]; };
      });
}
