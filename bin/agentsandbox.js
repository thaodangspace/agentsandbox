#!/usr/bin/env node
const { spawn } = require('child_process');
const path = require('path');
const os = require('os');

const isWin = os.platform() === 'win32';
const binName = isWin ? 'agentsandbox.exe' : 'agentsandbox';
const binPath = path.join(__dirname, '..', 'dist', binName);

const args = process.argv.slice(2);
const child = spawn(binPath, args, { stdio: 'inherit' });

child.on('close', (code) => {
  process.exit(code);
});
