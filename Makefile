.PHONY: build test install clean lint fmt run help

# Binary name
BINARY_NAME=agentsandbox

# Build directory
BUILD_DIR=bin

# Go parameters
CGO_ENABLED ?= 0
export CGO_ENABLED
GOCMD=go
GOBUILD=$(GOCMD) build
GOCLEAN=$(GOCMD) clean
GOTEST=$(GOCMD) test
GOGET=$(GOCMD) get
GOMOD=$(GOCMD) mod
GOFMT=$(GOCMD) fmt
GOVET=$(GOCMD) vet

# Main package path
MAIN_PATH=./cmd/agentsandbox

# Build the project
build:
	@echo "Building $(BINARY_NAME)..."
	@mkdir -p $(BUILD_DIR)
	$(GOBUILD) -o $(BUILD_DIR)/$(BINARY_NAME) $(MAIN_PATH)
	@echo "Build complete: $(BUILD_DIR)/$(BINARY_NAME)"

# Build for multiple platforms
build-all:
	@echo "Building for multiple platforms..."
	@mkdir -p $(BUILD_DIR)
	GOOS=linux GOARCH=amd64 $(GOBUILD) -o $(BUILD_DIR)/$(BINARY_NAME)-linux-amd64 $(MAIN_PATH)
	GOOS=linux GOARCH=arm64 $(GOBUILD) -o $(BUILD_DIR)/$(BINARY_NAME)-linux-arm64 $(MAIN_PATH)
	GOOS=darwin GOARCH=amd64 $(GOBUILD) -o $(BUILD_DIR)/$(BINARY_NAME)-darwin-amd64 $(MAIN_PATH)
	GOOS=darwin GOARCH=arm64 $(GOBUILD) -o $(BUILD_DIR)/$(BINARY_NAME)-darwin-arm64 $(MAIN_PATH)
	@echo "Multi-platform build complete"

# Run tests
test:
	@echo "Running tests..."
	$(GOTEST) -v ./...

# Run tests with coverage
test-coverage:
	@echo "Running tests with coverage..."
	$(GOTEST) -v -coverprofile=coverage.out ./...
	$(GOCMD) tool cover -html=coverage.out -o coverage.html
	@echo "Coverage report generated: coverage.html"

# Install the binary
install:
	@echo "Installing $(BINARY_NAME)..."
	$(GOCMD) install $(MAIN_PATH)
	@echo "Installed successfully"

# Clean build artifacts
clean:
	@echo "Cleaning..."
	$(GOCLEAN)
	rm -rf $(BUILD_DIR)
	rm -f coverage.out coverage.html
	@echo "Clean complete"

# Format code
fmt:
	@echo "Formatting code..."
	$(GOFMT) ./...
	@echo "Format complete"

# Run linter
lint:
	@echo "Running linter..."
	@which golangci-lint > /dev/null || (echo "golangci-lint not installed. Install with: curl -sSfL https://raw.githubusercontent.com/golangci/golangci-lint/master/install.sh | sh -s -- -b $$(go env GOPATH)/bin v1.54.2" && exit 1)
	golangci-lint run ./...
	@echo "Lint complete"

# Run go vet
vet:
	@echo "Running go vet..."
	$(GOVET) ./...
	@echo "Vet complete"

# Tidy dependencies
tidy:
	@echo "Tidying dependencies..."
	$(GOMOD) tidy
	@echo "Tidy complete"

# Run the application
run:
	@echo "Running $(BINARY_NAME)..."
	$(GOCMD) run $(MAIN_PATH)

# Run the application with arguments
run-with-args:
	@echo "Running $(BINARY_NAME) with arguments..."
	$(GOCMD) run $(MAIN_PATH) $(ARGS)

# Release build (with optimizations)
release:
	@echo "Building release version..."
	@mkdir -p $(BUILD_DIR)
	$(GOBUILD) -ldflags="-s -w" -o $(BUILD_DIR)/$(BINARY_NAME) $(MAIN_PATH)
	@echo "Release build complete: $(BUILD_DIR)/$(BINARY_NAME)"

# Help
help:
	@echo "Available targets:"
	@echo "  build          - Build the binary"
	@echo "  build-all      - Build for multiple platforms"
	@echo "  test           - Run tests"
	@echo "  test-coverage  - Run tests with coverage report"
	@echo "  install        - Install the binary"
	@echo "  clean          - Clean build artifacts"
	@echo "  fmt            - Format code"
	@echo "  lint           - Run linter"
	@echo "  vet            - Run go vet"
	@echo "  tidy           - Tidy dependencies"
	@echo "  run            - Run the application"
	@echo "  run-with-args  - Run with arguments (use ARGS=...)"
	@echo "  release        - Build optimized release version"
	@echo "  help           - Show this help message"

# Default target
.DEFAULT_GOAL := help
