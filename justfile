# List available commands
default:
    @just --list

# Build the project
build:
    go build -o mutagui .

# Run the application
run:
    go run .

# Run tests
test:
    go test ./...

# Check code with go vet
lint:
    go vet ./...

# Format code
format:
    gofmt -w .

# Check formatting
format-check:
    test -z "$(gofmt -l .)"

# Run all checks (format, lint, test)
check: format-check lint test

# Fix formatting
fix:
    gofmt -w .

# Clean build artifacts
clean:
    rm -f mutagui
    go clean

# Install the binary to GOPATH/bin
install:
    go install .

# Update dependencies
deps:
    go mod tidy
