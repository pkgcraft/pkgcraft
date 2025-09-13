# 0.0.29

## Added
- Support loading portage repos via global --portage option. Automatic portage
  repo loading functionality was previously dropped so this option will need to
  be used if registering repos with pkgcraft isn't done.

# 0.0.28

## Changed
- Update MSRV to 1.88.
- Migrate from git2 to gix for repo syncing support.

# 0.0.27

## Added
- pk pkg source: support specifying a number of runs for benchmarking
- pk pkg source: add -c/--cumulative option for performing multiple runs
- pk pkg env: support glob-based filters (#265)
- pk pkg showkw: provide a tabular view by default
- pk repo add: add initial support for adding repos to the config (#284)
- pk repo remove: add initial support for removing repos from the config (#284)
- pk repo sync: add initial repo syncing support (#284)
- pk repo list: add initial support for listing repos from the config

# 0.0.26

## Added
- pk: add --color to forcibly enable or disable color support
- pk completion: add shell completion generation support
- pk pkg showkw: add -a/--arches support using tri-state filtering
- pk pkg showkw: add -p/--prefix to filter prefix keywords
- pk repo metadata regen: add -o/--output support

## Changed
- pk pkg source: add short option -b for --bench and change short option -B for --bound

# 0.0.25

## Changed
- Bumped MSRV to 1.84.

## Fixed
- Respect jobs setting for metadata generation.
- Ignore custom profile-formats extensions for tools that don't use profiles.
- Fix repo initialization for tools that accept multiple alias targets.

# 0.0.24

## Added
- `pk repo revdeps`: support serializing reverse dependency cache to QA report format (#273).

# 0.0.23

## Added
- Add -i/--ignore option to ignore invalid packages for relevant subcommands.

## Fixed
- `pk pkg env`: Fix populating the external variables set.

# 0.0.22

## Added
- `pk pkg`: add short option -r/--repo for all subcommands

## Changed
- `pk pkg metadata`: change -r/--remove to -R/--remove
- Bail on ebuild repo initialization if custom profile-formats are used (#251).
- Respect custom cache location when generating metadata.

## Fixed
- `pk pkg metadata`: fix -o/--output handling with global task queue

# 0.0.21

## Added
- `pk repo eclass`: add repo eclass usage stats support
- `pk pkg env`: add --repo support
- `pk pkg fetch`: add -m/--mirrors to try fetching from default mirrors
- `pk pkg fetch`: add -p/--pretend to output targets instead of fetching
- `pk repo license`: add repo license usage stats support
- `pk pkg manifest`: add -m/--mirrors support to try fetching from default mirrors
- `pk repo mirror`: add repo mirror usage stats support
- `pk pkg pretend`: add --repo support
- `pk pkg source`: add --repo support

## Changed
- `pk pkg metadata`: use current working directory for repo by default
- Lazily load the system config for subcommands that may use it.
- `pk repo eapi`: subcommand rename from `eapis`

# 0.0.20

## Added
- `pk pkg fetch`: add initial support to download package distfiles in parallel
- `pk pkg manifest`: add initial support to download and manifest packages in parallel
- `pk pkg metadata`: add -r/--remove option to remove target entries
- Include generated completion for various shells in release tarballs at shell/*.

## Changed
- Bumped MSRV to 1.82.

# 0.0.19

## Fixed
- Fix global threadpool usage issues causing metadata generation to be roughly
  3x slower on machines with high core counts.

# 0.0.18

## Changed
- Parallelize package creation for relevant commands. This allows metadata
  generation to be run in parallel for serialized package iterators leading to
  large speed-ups when targeting repos with old or missing metadata caches.

- Most tools that iterate over a repo now log invalid package errors and will
  return an error code on exit if any package errors occur.

# 0.0.17

## Added
- `pk repo metadata regen`: --use-local option generates profiles/use.local.desc

## Changed
- Bumped MSRV to 1.80.

## Fixed
- Generate ebuild metadata in separate processes during package iteration to
  fix crash issues.

# 0.0.16

## Fixed
- Fix file descriptor handling in pkgcraft for rust-1.80.0 support.

# 0.0.15

## Changed
- Bumped MSRV to 1.76.
- Various documentation updates for subcommands and options.
- `pk pkg metadata`: Don't use internal targets when performing full repo scans.

# 0.0.14

## Added
- `pk cpv`: Add CPV-related support separate from `pk dep`. This provides much
  of the same support that `pk dep` provides for package dependencies, but
  instead for CPV objects (e.g. cat/pkg-1-r2 where a corresponding package
  dependency could be =cat/pkg-1-r2).

- `pk pkg showkw`: Add initial package keyword output support. This command is
  the rough precursor to a `pkgdev showkw` and eshowkw alternative that
  currently doesn't support tabular output.

- `pk pkg env`: Add initial ebuild package environment dumping support. This
  command sources targeted ebuilds and dumps their respective bash environments
  to stdout.

- `pk pkg metadata`: Add initial support for selective package metadata
  mangling currently only allowing regeneration and verification. Where `pk
  repo metadata` operates on entire ebuild repos, this command operates on
  custom restrictions such as paths or package globs (by default it uses the
  current working directory). This allows much quicker, targeted metadata
  generation when working in specific packages directories or for scripts
  targeting specific packages.

## Changed
- `pk repo metadata`: Split actions into separate subcommands so the previous
  default regen action now must be run via `pk repo metadata regen`. Cache
  cleaning and removal are supported via the `clean` and `remove` subcommands,
  respectively.

# 0.0.13

## Fixed
- `pk repo metadata`: use proper package prefixes for failure messages

# 0.0.12

## Changed
- `pk dep parse`: convert --eapi value during arg parsing
- `pk repo metadata`: set default target repo to the current directory
- `pk repo metadata`: add -n/--no-progress option to disable progress bar (#140)

## Fixed
- Skip loading system config files during tests.
- Fix error propagation for utilities running in parallel across process pools.

# 0.0.11

## Added
- `pk pkg revdeps`: initial support for querying reverse dependencies

## Changed
- Bumped MSRV to 1.70.

## Fixed
- `pk repo metadata`: remove outdated cache entries

# 0.0.10

## Added
- Support loading the config from a custom path and disabling config loading
  via the `PKGCRAFT_NO_CONFIG` environment variable (#115).

## Fixed
- `pk repo metadata`: ignore `declare` errors with unset variables

# 0.0.9

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

# 0.0.8

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

# 0.0.7

- initial release
