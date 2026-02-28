set allow-duplicate-recipes := true
set allow-duplicate-variables := true
set shell := ["bash", "-euo", "pipefail", "-c"]

# ---------------------------------------------------------------------------- #
#                                 DEPENDENCIES                                 #
# ---------------------------------------------------------------------------- #

# Rust: https://rust-lang.org/tools/install
cargo := require("cargo")
rustc := require("rustc")

# ---------------------------------------------------------------------------- #
#                                    RECIPES                                   #
# ---------------------------------------------------------------------------- #

# Show available commands
default:
    @just --list

# Build the program
build:
    anchor build

# Run all code checks
full-check:
    cargo fmt --all --check
    cargo clippy -- --deny warnings
alias fc := full-check

full-write:
    cargo fmt --all
alias fw := full-write

# Run tests
test:
    anchor test

# ---------------------------------------------------------------------------- #
#                              PROGRAM COMMANDS                                 #
# ---------------------------------------------------------------------------- #

# Build only the main stablecoin program
build-program:
    cargo build-sbf --manifest-path programs/solana-stablecoin-standard/Cargo.toml

# Build only the compliance hook program  
build-hook:
    cargo build-sbf --manifest-path programs/sss-compliance-hook/Cargo.toml

# Build all programs (default)
build-all: build-program build-hook

# Test only the main program
test-program:
    cargo test --manifest-path programs/solana-stablecoin-standard/Cargo.toml

# Test only the compliance hook
test-hook:
    cargo test --manifest-path programs/sss-compliance-hook/Cargo.toml

# ---------------------------------------------------------------------------- #
#                               BACKEND COMMANDS                               #
# ---------------------------------------------------------------------------- #

# Install backend dependencies
backend-deps:
    cd backend && cargo fetch

# Build backend
backend-build:
    cd backend && cargo build --release

# Run backend in development mode
backend-run:
    cd backend && cargo run

# Run backend in release mode
backend-release:
    cd backend && cargo run --release

# Build and run backend with Docker
backend-docker-build:
    cd backend && docker build -t sss-backend .

backend-docker-run:
    docker run -p 3000:3000 --env-file backend/.env sss-backend

# Full stack: build programs + build backend
full-stack: build-all backend-build
alias fs := full-stack

# Dev stack: build programs + run backend in dev mode
dev: build-all backend-run