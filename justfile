# Only check staged files
check:
    #!/usr/bin/env fish
    git stash push -uk -m "just-check-stash" &>/dev/null
    
    set -l failed 0
    cargo fmt --check; or set failed 1
    cargo clippy; or set failed 1
    
    git stash list | rg -q "just-check-stash" && git stash pop &>/dev/null
    
    exit $failed

format:
    cargo fmt

build:
    cargo build
