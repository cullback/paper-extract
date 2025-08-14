check:
    #!/usr/bin/env fish
    set -l failed 0
    cargo fmt --check; or set failed 1
    cargo clippy; or set failed 1
    exit $failed

format:
    cargo fmt

build:
    cargo build
