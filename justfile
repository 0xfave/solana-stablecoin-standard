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

# Run backend tests
backend-test:
	cd backend && cargo test

# Run backend integration tests (requires devnet)
backend-integration-test:
	cd backend && cargo test integration_tests

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

# SDK: build TypeScript SDK
sdk-build:
	cd sdk && yarn install && yarn build
	@echo "SDK built at sdk/dist/"

# SDK: run SDK tests
sdk-test:
	cd sdk && yarn test

# SDK: all (install + build)
sdk: sdk-build

# ---------------------------------------------------------------------------- #
#                                CLI COMMANDS                                   #
# ---------------------------------------------------------------------------- #

# Build CLI
cli-build:
	cd cli && RUSTFLAGS="-A dead_code" cargo build --release

# Run CLI with arguments
cli *args:
	cd cli && cargo run --release -- {{args}}

# Initialize stablecoin with preset
cli-init preset:
	cd cli && cargo run --release -- init --preset {{preset}}

# CLI status
cli-status:
	cd cli && cargo run --release -- status

# CLI supply
cli-supply:
	cd cli && cargo run --release -- supply

# CLI mint tokens
cli-mint recipient amount:
	cd cli && cargo run --release -- mint {{recipient}} {{amount}}

# CLI burn tokens
cli-burn amount:
	cd cli && cargo run --release -- burn {{amount}}

# CLI freeze account
cli-freeze address:
	cd cli && cargo run --release -- freeze {{address}}

# CLI thaw account
cli-thaw address:
	cd cli && cargo run --release -- thaw {{address}}

# CLI pause/unpause
cli-pause:
	cd cli && cargo run --release -- pause

cli-unpause:
	cd cli && cargo run --release -- unpause

# CLI blacklist
cli-blacklist-add address:
	cd cli && cargo run --release -- blacklist add {{address}}

cli-blacklist-remove address:
	cd cli && cargo run --release -- blacklist remove {{address}}

# CLI seize
cli-seize address to amount:
	cd cli && cargo run --release -- seize {{address}} --to {{to}} {{amount}}

# CLI minters
cli-minters-list:
	cd cli && cargo run --release -- minters list

cli-minters-add address:
	cd cli && cargo run --release -- minters add {{address}}

cli-minters-remove address:
	cd cli && cargo run --release -- minters remove {{address}}

# CLI holders
cli-holders:
	cd cli && cargo run --release -- holders

# Full stack: build programs + build backend
full-stack: build-all backend-build
alias fs := full-stack

# Dev stack: build programs + run backend in dev mode
dev: build-all backend-run