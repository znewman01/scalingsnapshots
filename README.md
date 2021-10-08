# Scaling Snapshots

## Setup

### With Nix

For development (if you have `[direnv]` installed, just `cd` into this directory):

``` shell
$ nix-shell
```

To compile everything:

``` shell
$ nix build
$ ./result/bin/sssim  # the simulator binary, compiled from Rust
Hello, world!
$ ./result/bin/ssanalyze --help  # the analysis Python script, for charts etc.
```

And to run "end-to-end" (a simple example that hooks up all the components):

``` shell
$ nix build -f nix run
$ ls result/
hello.png
```

[`direnv`]: https://direnv.net/

## Otherwise

### Rust

Install [`rustup`], and use that to install the appropriate Rust version. Then:

``` shell
$ cargo run
```

[`rustup`]: https://rustup.rs/

### Python

Install [`poetry`]. Then (in `analysis/` directory) run `poetry install`, and
one of:

``` shell
$ poetry shell  # create a new shell with appropriate virtualenv
$ poetry run pytest  # run tests
$ poetry run python -m ssanalyze -h # run help
```

(And analogously for `logparser/` directory and `sslogs`.)

[`poetry`]: https://python-poetry.org/
