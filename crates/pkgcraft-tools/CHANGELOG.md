- unreleased

  - `pk repo metadata`: show progress bar during cache validation phase

  - Exit as expected when a SIGPIPE occurs (#112).

  - `pk pkg source`: `-j/--jobs` defaults to # of physical CPUs

  - `pk pkg source`: support sorting in ascending order via `--sort`

  - `pk pkg`: source ebuild from the current working directory by default

  - `pk pkg`: add support for path-based targets

  - `pk pkg source`: use human-time duration for `--bench` args

- 0.0.8 (2023-06-11)

  - `pk pkg source`: support multiple `-b/--bound` args

  - `pk pkg`: loop over targets performing a run for each

  - Check for configured repos before trying to load one from a path.

  - `pk pkg source`: match against all configured ebuild repos by default

  - `pk repo metadata`: support multiple repo targets

  - `pk repo metadata`: change `-r/--repo` option into a positional argument

  - Apply bounds to `-j/--jobs` args to be a positive integer that's less than or
    equal to a system's logical CPUs.

- 0.0.7 (2023-06-04)

  - initial release
