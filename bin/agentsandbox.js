#!/usr/bin/env node
const { spawn } = require('child_process');
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

const binPath = path.join(__dirname, '..', 'dist', binName);

const args = process.argv.slice(2);
const child = spawn(binPath, args, { stdio: 'inherit' });

child.on('close', (code) => {
    process.exit(code);
});
