precommit:
    cargo +1.85 fmt --all
    cargo +1.85 clippy --all-targets -- -D warnings
    cargo +1.85 test --all-targets