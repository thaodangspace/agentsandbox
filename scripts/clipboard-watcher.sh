#!/bin/bash
# clipboard-watcher.sh - Monitor X11 clipboard for images and save them to a shared directory

# Exit on error
set -e

# Default clipboard directory
CLIPBOARD_DIR="${CLIPBOARD_DIR:-$HOME/.config/agentsandbox/clipboard}"

# Create clipboard directory if it doesn't exist
mkdir -p "$CLIPBOARD_DIR"

# Maximum number of clipboard images to keep (to prevent disk bloat)
MAX_IMAGES="${MAX_IMAGES:-10}"

# Logging
LOG_FILE="$HOME/.config/agentsandbox/clipboard_watcher.log"
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "$LOG_FILE"
}

# Check if xclip is available
if ! command -v xclip &> /dev/null; then
    log "ERROR: xclip is not installed. Please install it: sudo apt-get install xclip"
    echo "ERROR: xclip is not installed. Please install it: sudo apt-get install xclip" >&2
    exit 1
fi

# Check if DISPLAY is set
if [ -z "$DISPLAY" ]; then
    log "ERROR: DISPLAY environment variable is not set"
    echo "ERROR: DISPLAY environment variable is not set" >&2
    exit 1
fi

log "Clipboard watcher started"
log "Monitoring clipboard for images, saving to: $CLIPBOARD_DIR"

# Function to clean up old images
cleanup_old_images() {
    # Count non-symlink image files
    local count=$(find "$CLIPBOARD_DIR" -maxdepth 1 -type f \( -name "clipboard-*.png" -o -name "clipboard-*.jpg" -o -name "clipboard-*.jpeg" \) | wc -l)

    if [ "$count" -gt "$MAX_IMAGES" ]; then
        log "Cleaning up old images (found $count, keeping $MAX_IMAGES)"
        # Delete oldest files, keeping MAX_IMAGES
        find "$CLIPBOARD_DIR" -maxdepth 1 -type f \( -name "clipboard-*.png" -o -name "clipboard-*.jpg" -o -name "clipboard-*.jpeg" \) -printf '%T+ %p\n' | \
            sort | \
            head -n -"$MAX_IMAGES" | \
            cut -d' ' -f2- | \
            xargs -r rm -f
    fi
}

# Keep track of last clipboard content hash to avoid duplicate saves
LAST_HASH=""

# Main monitoring loop
while true; do
    # Check if clipboard contains image data
    # xclip -selection clipboard -t TARGETS returns list of available formats
    if xclip -selection clipboard -t TARGETS -o 2>/dev/null | grep -q "image/"; then
        # Determine image format
        if xclip -selection clipboard -t TARGETS -o 2>/dev/null | grep -q "image/png"; then
            FORMAT="png"
            MIME_TYPE="image/png"
        elif xclip -selection clipboard -t TARGETS -o 2>/dev/null | grep -q "image/jpeg"; then
            FORMAT="jpg"
            MIME_TYPE="image/jpeg"
        else
            # Try PNG as fallback
            FORMAT="png"
            MIME_TYPE="image/png"
        fi

        # Get clipboard content and compute hash
        TEMP_FILE=$(mktemp)
        if xclip -selection clipboard -t "$MIME_TYPE" -o > "$TEMP_FILE" 2>/dev/null; then
            CURRENT_HASH=$(md5sum "$TEMP_FILE" | cut -d' ' -f1)

            # Only save if this is a new image (different from last saved)
            if [ "$CURRENT_HASH" != "$LAST_HASH" ]; then
                TIMESTAMP=$(date '+%Y%m%d-%H%M%S')
                FILENAME="clipboard-${TIMESTAMP}.${FORMAT}"
                FILEPATH="$CLIPBOARD_DIR/$FILENAME"

                # Save the image
                mv "$TEMP_FILE" "$FILEPATH"
                log "Saved clipboard image: $FILENAME (hash: $CURRENT_HASH)"

                # Update symlink to latest image
                ln -sf "$FILENAME" "$CLIPBOARD_DIR/latest.${FORMAT}"

                # Also create a generic 'latest' symlink (for easier access)
                ln -sf "$FILENAME" "$CLIPBOARD_DIR/latest"

                # Update last hash
                LAST_HASH="$CURRENT_HASH"

                # Cleanup old images
                cleanup_old_images
            else
                rm -f "$TEMP_FILE"
            fi
        else
            rm -f "$TEMP_FILE"
        fi
    fi

    # Sleep for a short interval before checking again
    sleep 0.5
done
