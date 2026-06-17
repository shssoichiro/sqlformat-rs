precommit:
    cargo +1.86 fmt --all
    cargo +1.86 clippy --all-targets -- -D warnings
    cargo +1.86 test --all-targets
    cargo +1.86 bench --no-run