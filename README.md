# es-system
EmulationStation system/features config generator for REG-Linux (Rust)

## Build-time mode

Positional-argument mode, invoked from `es-system.mk` in the REG-Linux tree. It
generates `rs_systems.cfg` and `rs_features.json` plus the translation headers
and the `roms/` skeleton:

```
es-system [--format xml|json] <yml> <features> <es_translations> <es_keys_translations> \
          <keys_parent_folder> <blacklisted_words> <config> \
          <es_systems> <es_features> <gen_defaults_global> \
          <gen_defaults_arch> <romsdirsource> <romsdirtarget> <arch>
```

Defaults to `--format xml`.

## On-device regeneration

```
es-system regenerate [--user-dir DIR] [--data-dir DIR] [--output-dir DIR] \
                     [--launcher-dir DIR] [--config FILE] [--arch ARCH] [--format xml|json]
```

Manual only — nothing on a REG-Linux device invokes this, at boot or otherwise.
Defaults to `--format json`, the only features format REG-Station reads.

Each input resolves `--user-dir` first and `--data-dir` second, so the command
works unmodified on a stock image (producing byte-identical output to that
image's build) and the user overrides only the file they edited:

| Input | Default `--user-dir` | Default `--data-dir` |
|---|---|---|
| `es_systems.yml` | `/userdata/system/configs/es-system` | `/usr/share/es-system` |
| `es_features.yml` | same | same |
| `installed-packages.conf` | same | same |
| `arch.conf` | same | same |

`installed-packages.conf` is the `BR2_PACKAGE_*=y` set the image was built with;
without it every `requireAnyOf` gate fails closed, so a missing one is an error
rather than an empty system list.

`launcher-defaults.yml` / `launcher-defaults-arch.yml` come from `--launcher-dir`
(`/usr/share/regmsg/launcher`) with no user override, because regmsgd reads them
from that same path — a private copy would let the frontend and the launcher
disagree about which emulator a system uses.

Output goes to `--output-dir` (`/userdata/system/configs/regstation`), which
REG-Station and regmsgd both prefer over `/usr/share/regstation`.
