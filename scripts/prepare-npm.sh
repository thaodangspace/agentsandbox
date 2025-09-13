#!/bin/bash

# Prepare script for npm publishing
# This script builds all required binaries for npm distribution

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_info "Preparing binaries for npm publishing..."

# Clean dist directory
rm -rf dist
mkdir -p dist

# Check if we're in CI or have GitHub Actions available
if [[ -n "$GITHUB_ACTIONS" ]]; then
    print_info "Running in GitHub Actions - all platforms should be available"
    
    # In CI, we expect binaries to be built by the workflow
    # This script would be run after artifacts are downloaded
    print_info "Checking for required binaries..."
    
    required_files=(
        "agentsandbox-linux-x64"
        "agentsandbox-darwin-x64"
        "agentsandbox-darwin-arm64"
    )
    
    missing_files=()
    for file in "${required_files[@]}"; do
        if [[ ! -f "dist/$file" ]]; then
            missing_files+=("$file")
        fi
    done
    
    if [[ ${#missing_files[@]} -gt 0 ]]; then
        print_error "Missing required binaries for npm publishing:"
        for file in "${missing_files[@]}"; do
            print_error "  - dist/$file"
        done
        exit 1
    fi
    
    print_info "All required binaries found!"
    
else
    print_info "Building locally - attempting to build all targets..."
    
    # Try to build all targets locally
    ./scripts/build.sh --npm
    
    # Check what we actually built
    built_files=()
    if [[ -f "dist/agentsandbox-linux-x64" ]]; then
        built_files+=("Linux x64")
    fi
    if [[ -f "dist/agentsandbox-darwin-x64" ]]; then
        built_files+=("macOS x64")
    fi
    if [[ -f "dist/agentsandbox-darwin-arm64" ]]; then
        built_files+=("macOS ARM64")
    fi
    
    print_info "Successfully built for: ${built_files[*]}"
    
    if [[ ${#built_files[@]} -lt 2 ]]; then
        print_warning "Only ${#built_files[@]} platform(s) built locally"
        print_warning "For complete npm publishing, consider using GitHub Actions workflow"
        print_warning "which builds all platforms automatically"
    fi
fi

# Verify all binaries are executable and work
print_info "Verifying binaries..."

for binary in dist/*; do
    if [[ -f "$binary" && -x "$binary" ]]; then
        binary_name=$(basename "$binary")
        print_info "✓ $binary_name is executable"
        
        # Quick test to make sure it runs
        if "$binary" --version &>/dev/null; then
            print_info "✓ $binary_name runs correctly"
        else
            print_warning "⚠ $binary_name may have issues (--version failed)"
        fi
    fi
done

print_info "Npm preparation complete!"
print_info "Files ready for publishing:"
ls -la dist/

print_info ""
print_info "To publish to npm:"
print_info "  npm publish"
print_info ""
print_info "To test locally:"
print_info "  npm pack"
print_info "  npm install -g agentsandbox-*.tgz"
