#!/bin/bash
# container-clipboard-helper.sh - Helper utilities for accessing clipboard images inside the container

CLIPBOARD_DIR="/workspace/.clipboard"

# Get the path to the latest clipboard image
get_latest() {
    if [ ! -d "$CLIPBOARD_DIR" ]; then
        echo "Error: Clipboard directory not found at $CLIPBOARD_DIR" >&2
        return 1
    fi

    # Check for the 'latest' symlink first
    if [ -L "$CLIPBOARD_DIR/latest" ]; then
        echo "$CLIPBOARD_DIR/latest"
        return 0
    fi

    # Otherwise, find the most recent clipboard image
    local latest=$(find "$CLIPBOARD_DIR" -maxdepth 1 -type f \( -name "clipboard-*.png" -o -name "clipboard-*.jpg" -o -name "clipboard-*.jpeg" \) -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -n 1 | cut -d' ' -f2-)

    if [ -z "$latest" ]; then
        echo "No clipboard images found in $CLIPBOARD_DIR" >&2
        return 1
    fi

    echo "$latest"
    return 0
}

# List all clipboard images
list_all() {
    if [ ! -d "$CLIPBOARD_DIR" ]; then
        echo "Error: Clipboard directory not found at $CLIPBOARD_DIR" >&2
        return 1
    fi

    find "$CLIPBOARD_DIR" -maxdepth 1 -type f \( -name "clipboard-*.png" -o -name "clipboard-*.jpg" -o -name "clipboard-*.jpeg" \) -printf '%T@ %p\n' | sort -rn | cut -d' ' -f2-
}

# Show help
show_help() {
    cat <<EOF
Clipboard Helper - Access clipboard images from the container

Usage: clipboard [command]

Commands:
    latest      Get the path to the most recent clipboard image (default)
    list        List all clipboard images
    help        Show this help message

Examples:
    # Get latest clipboard image path
    clipboard
    clipboard latest

    # Use with Claude Code
    claude code \$(clipboard)

    # List all clipboard images
    clipboard list
EOF
}

# Main logic
case "${1:-latest}" in
    latest)
        get_latest
        ;;
    list)
        list_all
        ;;
    help|--help|-h)
        show_help
        ;;
    *)
        echo "Unknown command: $1" >&2
        echo "Run 'clipboard help' for usage information" >&2
        exit 1
        ;;
esac
