const fs = require('fs');
const path = require('path');
const os = require('os');

const platform = os.platform();
const arch = os.arch();

let binName;
if (platform === 'win32') {
    binName = 'agentsandbox.exe';
} else if (platform === 'linux' && arch === 'x64') {
    binName = 'agentsandbox-linux-x64';
} else if (platform === 'darwin' && arch === 'x64') {
    binName = 'agentsandbox-darwin-x64';
} else if (platform === 'darwin' && arch === 'arm64') {
    binName = 'agentsandbox-darwin-arm64';
} else {
    console.error(`Unsupported platform: ${platform}-${arch}`);
    console.error('Currently supports: linux-x64, darwin-x64, darwin-arm64, win32');
    process.exit(1);
}

const binPath = path.join(__dirname, 'dist', binName);

// Check if binary exists
if (!fs.existsSync(binPath)) {
    console.error(`Pre-built binary not found: ${binPath}`);
    console.error('This package should include pre-built binaries.');
    process.exit(1);
}

// Ensure binary is executable on Unix-like systems
if (platform !== 'win32') {
    fs.chmodSync(binPath, 0o755);
}
