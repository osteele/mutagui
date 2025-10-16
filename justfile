# List available commands
default:
    @just --list

# Build the project in release mode
build:
    cargo build --release

# Build the project in debug mode
build-debug:
    cargo build

# Run the application
run:
    cargo run

# Run the application in release mode
run-release:
    cargo run --release

# Run tests
test:
    cargo test

# Check code with clippy
lint:
    cargo clippy -- -D warnings

# Format code
format:
    cargo fmt

# Check formatting
format-check:
    cargo fmt -- --check

# Run all checks (format, lint, test)
check: format-check lint test

# Fix formatting and linting issues
fix:
    cargo fmt
    cargo clippy --fix --allow-staged --allow-dirty

# Clean build artifacts
clean:
    cargo clean

# Install the binary to cargo bin directory
install:
    cargo install --path .
