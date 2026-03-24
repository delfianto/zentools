set shell := ["bash", "-euo", "pipefail", "-c"]

bin_name := "zen"
install_dir := env("HOME") / ".local" / "bin"

# Default: show available recipes
default:
    @just --list

# Run all checks (lint + compile + test)
check: lint build-check test

# Run clippy linter with all warnings as errors
lint:
    cargo clippy -- -W clippy::all -D warnings

# Check compilation without producing binary
build-check:
    cargo check

# Run all tests
test:
    cargo test

# Run tests with output shown
test-verbose:
    cargo test -- --nocapture

# Build release binary
build:
    cargo build --release

# Build and show binary info
build-info: build
    @ls -lh target/release/{{ bin_name }}
    @file target/release/{{ bin_name }}

# Install to ~/.local/bin with epp and smu symlinks
install: build
    @mkdir -p {{ install_dir }}
    cp target/release/{{ bin_name }} {{ install_dir }}/{{ bin_name }}
    @chmod +x {{ install_dir }}/{{ bin_name }}
    @# Create busybox-style symlinks
    ln -sf {{ bin_name }} {{ install_dir }}/epp
    ln -sf {{ bin_name }} {{ install_dir }}/smu
    ln -sf {{ bin_name }} {{ install_dir }}/mem
    @echo "Installed to {{ install_dir }}:"
    @ls -la {{ install_dir }}/{{ bin_name }} {{ install_dir }}/epp {{ install_dir }}/smu {{ install_dir }}/mem

# Uninstall from ~/.local/bin
uninstall:
    rm -f {{ install_dir }}/{{ bin_name }} {{ install_dir }}/epp {{ install_dir }}/smu {{ install_dir }}/mem
    @echo "Removed zen, epp, smu, mem from {{ install_dir }}"

# Clean build artifacts
clean:
    cargo clean

# Format code
fmt:
    cargo fmt

# Check formatting without changing files
fmt-check:
    cargo fmt -- --check
