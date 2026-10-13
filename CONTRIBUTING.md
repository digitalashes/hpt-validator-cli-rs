# Contributing

Open an issue before a large change. Validation belongs in
[hpt-validator](https://github.com/digitalashes/hpt-validator-rs); this repository only
renders its results. Check it out next to this one, as `Cargo.toml` and the tests expect,
then a pull request passes:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Output without `--extended` equals upstream's CLI: `oracle/run.sh` (Node 20.19.4) runs it
on every case and compares both output formats.
