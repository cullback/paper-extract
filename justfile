# Only check staged files
check:
    -@git stash push -uk -m "just-check-stash" &>/dev/null
    -cargo fmt --check
    -cargo clippy
    -@git stash list | rg -q "just-check-stash" && git stash pop &>/dev/null

format:
    cargo fmt

build:
    cargo build
