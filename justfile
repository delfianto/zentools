bin     := "zentools"
bin_dir := env_var("HOME") / ".local/bin"
sys_dir := "/usr/local/bin"

# List available recipes
default:
    @just --list

# Build release binary
build:
    cargo build --release

# Run tests
test:
    cargo test

# Auto-format the tree
fmt:
    cargo fmt --all

# Check formatting (CI gate)
fmt-check:
    cargo fmt --all -- --check

# Lint — warnings denied (CI gate)
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Full local gate, mirrors CI
check: fmt-check lint test

# Install into ~/.local/bin (default) or /usr/local/bin (--system, via sudo),
# with busybox-style zen-epp / zen-smu / zen-mem symlinks alongside the binary.
install *flags: build
    #!/usr/bin/env bash
    set -euo pipefail
    dir="{{bin_dir}}"
    sudo=""
    for f in {{flags}}; do
        case "$f" in
            --system) dir="{{sys_dir}}"; sudo="sudo" ;;
            *) echo "install: unknown flag '$f' (only --system is supported)" >&2; exit 1 ;;
        esac
    done
    $sudo install -Dm755 target/release/{{bin}} "$dir/{{bin}}"
    for tool in zen-epp zen-smu zen-mem; do
        $sudo ln -sf "{{bin}}" "$dir/$tool"   # relative target -> sibling {{bin}}
    done
    echo "installed $dir/{{bin}} (+ zen-epp, zen-smu, zen-mem)"

# Remove installed binary + symlinks (pass --system for /usr/local/bin via sudo)
uninstall *flags:
    #!/usr/bin/env bash
    set -euo pipefail
    dir="{{bin_dir}}"
    sudo=""
    for f in {{flags}}; do
        case "$f" in
            --system) dir="{{sys_dir}}"; sudo="sudo" ;;
            *) echo "uninstall: unknown flag '$f' (only --system is supported)" >&2; exit 1 ;;
        esac
    done
    $sudo rm -f "$dir/{{bin}}" "$dir/zen-epp" "$dir/zen-smu" "$dir/zen-mem"
    echo "removed $dir/{{bin}} and zen-* symlinks"

# Remove build artifacts
clean:
    cargo clean
