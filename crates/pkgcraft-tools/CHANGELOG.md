# 0.0.10 (2023-06-23)

## Added
- Support loading the config from a custom path and disabling config loading
  via the `PKGCRAFT_NO_CONFIG` environment variable (#115).

## Fixed
- `pk repo metadata`: ignore `declare` errors with unset variables

# 0.0.9 (2023-06-17)

## Added
- `pk pkg`: add support for path-based targets
- `pk pkg source`: support sorting in ascending order via `--sort`
- `pk repo eapis`: add subcommand to show EAPI usage
- `pk repo leaf`: add subcommand to output leaf packages
- `pk repo metadata`: show progress bar during cache validation phase

## Changed
- Using stdin with relevant commands requires an initial arg of `-`.
- Log events are written to stderr instead of stdout.
- `pk pkg`: source ebuild from the current working directory by default
- `pk pkg source`
  - `-j/--jobs` defaults to # of physical CPUs
  - use human-time duration for `--bench` args

## Fixed
- Exit as expected when a SIGPIPE occurs (#112).

# 0.0.8 (2023-06-11)

## Added
- `pk pkg source`: support multiple `-b/--bound` args
- `pk repo metadata`: support multiple repo targets

## Changed
- Apply bounds to `-j/--jobs` args to be a positive integer that's less than or
  equal to a system's logical CPUs.
- Check for configured repos before trying to load one from a path.
- `pk pkg`: loop over targets performing a run for each
- `pk pkg source`: match against all configured ebuild repos by default
- `pk repo metadata: change `-r/--repo` option into a positional argument

# 0.0.7 (2023-06-04)

  - initial release
