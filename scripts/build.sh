#!/bin/bash

# Build script for agentsandbox
# This script helps with local development and building

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Default to native build
TARGET=""
RELEASE_MODE=""
NPM_BUILD=false
COPY_TO_DIST=false

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --target)
            TARGET="$2"
            shift 2
            ;;
        --release)
            RELEASE_MODE="--release"
            shift
            ;;
        --npm)
            NPM_BUILD=true
            RELEASE_MODE="--release"
            COPY_TO_DIST=true
            shift
            ;;
        --dist)
            COPY_TO_DIST=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [options]"
            echo "Options:"
            echo "  --target TARGET    Specify target triple (e.g., x86_64-apple-darwin)"
            echo "  --release          Build in release mode"
            echo "  --npm              Build for npm distribution (Linux x64 + macOS, release mode, copy to dist)"
            echo "  --dist             Copy binaries to dist/ directory with npm naming"
            echo "  --help, -h         Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0                                    # Build for native target in debug mode"
            echo "  $0 --release                          # Build for native target in release mode"
            echo "  $0 --npm                              # Build for npm publishing (multiple targets)"
            echo "  $0 --target x86_64-unknown-linux-gnu # Cross-compile for Linux"
            echo ""
            echo "Note: Cross-compilation to macOS from Linux requires osxcross toolchain."
            echo "See: https://github.com/tpoechtrager/osxcross"
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Function to check if target toolchain is available
check_toolchain() {
    local target=$1
    case $target in
        x86_64-apple-darwin)
            command -v x86_64-apple-darwin14-clang &> /dev/null
            ;;
        aarch64-apple-darwin)
            command -v aarch64-apple-darwin20.4-clang &> /dev/null
            ;;
        *)
            true  # Assume other targets are available
            ;;
    esac
}

# Function to build for a specific target
build_target() {
    local target=$1
    local mode=$2
    
    # Check if target is macOS and we're on Linux
    if [[ "$target" == *"apple-darwin"* ]] && [[ "$(uname)" == "Linux" ]]; then
        if ! check_toolchain "$target"; then
            print_warning "Cross-compiling to macOS from Linux requires osxcross toolchain"
            print_warning "This is complex to set up. For npm publishing, we recommend:"
            print_warning "1. Use GitHub Actions: 'git tag v0.x.x && git push origin v0.x.x'"
            print_warning "2. Download artifacts from the GitHub workflow"
            print_warning "3. Use the npm-package artifact for publishing"
            print_warning ""
            print_warning "See CROSS_COMPILATION_SETUP.md for local setup instructions"
            print_warning "See NPM_PUBLISHING.md for the recommended workflow"
            >&2 echo "ERROR: macOS cross-compilation toolchain not available"
            return 1
        fi
    fi
    
    cargo build $mode --target "$target"
    local build_result=$?
    
    if [[ $build_result -ne 0 ]]; then
        return 1
    fi
    
    # Get binary path
    local binary_name="agentsandbox"
    if [[ "$target" == *"windows"* ]]; then
        binary_name="agentsandbox.exe"
    fi
    
    if [[ -n "$mode" ]]; then
        echo "target/$target/release/$binary_name"
    else
        echo "target/$target/debug/$binary_name"
    fi
}

# Function to copy binary to dist with npm naming
copy_to_dist() {
    local binary_path=$1
    local target=$2
    
    mkdir -p dist
    
    local dist_name
    case $target in
        x86_64-unknown-linux-gnu)
            dist_name="agentsandbox-linux-x64"
            ;;
        x86_64-apple-darwin)
            dist_name="agentsandbox-darwin-x64"
            ;;
        aarch64-apple-darwin)
            dist_name="agentsandbox-darwin-arm64"
            ;;
        x86_64-pc-windows-msvc)
            dist_name="agentsandbox.exe"
            ;;
        *)
            # For native builds, detect current platform
            local platform=$(uname -s | tr '[:upper:]' '[:lower:]')
            local arch=$(uname -m)
            case "$platform-$arch" in
                linux-x86_64)
                    dist_name="agentsandbox-linux-x64"
                    ;;
                darwin-x86_64)
                    dist_name="agentsandbox-darwin-x64"
                    ;;
                darwin-arm64)
                    dist_name="agentsandbox-darwin-arm64"
                    ;;
                *)
                    dist_name="agentsandbox-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"
                    ;;
            esac
            ;;
    esac
    
    print_info "Copying to dist/$dist_name"
    cp "$binary_path" "dist/$dist_name"
    chmod +x "dist/$dist_name"
    ls -lh "dist/$dist_name"
}

# Main build logic
if [[ "$NPM_BUILD" == true ]]; then
    print_info "Building for npm distribution..."
    
    # Define targets for npm distribution
    targets=("x86_64-unknown-linux-gnu" "x86_64-apple-darwin")
    
    # Add ARM64 macOS if we're on macOS or have the toolchain
    if [[ "$(uname)" == "Darwin" ]] || check_toolchain "aarch64-apple-darwin"; then
        targets+=("aarch64-apple-darwin")
    fi
    
    built_targets=()
    failed_targets=()
    
    for target in "${targets[@]}"; do
        print_info "Building for $target..."
        if binary_path=$(build_target "$target" "$RELEASE_MODE"); then
            if [[ -f "$binary_path" ]]; then
                copy_to_dist "$binary_path" "$target"
                built_targets+=("$target")
            else
                print_error "Binary not found: $binary_path"
                failed_targets+=("$target")
            fi
        else
            failed_targets+=("$target")
        fi
    done
    
    print_info "NPM build summary:"
    for target in "${built_targets[@]}"; do
        print_info "✓ Successfully built: $target"
    done
    
    if [[ ${#failed_targets[@]} -gt 0 ]]; then
        for target in "${failed_targets[@]}"; do
            print_warning "✗ Failed to build: $target"
        done
        print_warning "Some targets failed. Consider using GitHub Actions for complete cross-platform builds."
    fi
    
elif [[ -n "$TARGET" ]]; then
    # Build for specific target
    if binary_path=$(build_target "$TARGET" "$RELEASE_MODE"); then
        if [[ -f "$binary_path" ]]; then
            print_info "Build completed successfully!"
            print_info "Binary location: $binary_path"
            ls -lh "$binary_path"
            
            if [[ "$COPY_TO_DIST" == true ]]; then
                copy_to_dist "$binary_path" "$TARGET"
            fi
        else
            print_error "Binary not found: $binary_path"
            exit 1
        fi
    else
        print_error "Build failed for target: $TARGET"
        exit 1
    fi
else
    # Build for native target
    print_info "Building for native target"
    cargo build $RELEASE_MODE
    
    # Determine binary path
    if [[ -n "$RELEASE_MODE" ]]; then
        BINARY_PATH="target/release/agentsandbox"
    else
        BINARY_PATH="target/debug/agentsandbox"
    fi
    
    if [[ -f "$BINARY_PATH" ]]; then
        print_info "Build completed successfully!"
        print_info "Binary location: $BINARY_PATH"
        ls -lh "$BINARY_PATH"
        
        if [[ "$COPY_TO_DIST" == true ]]; then
            copy_to_dist "$BINARY_PATH" ""
        fi
    else
        print_error "Binary not found: $BINARY_PATH"
        exit 1
    fi
fi
