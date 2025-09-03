const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');
const os = require('os');

const targetDir = path.join(__dirname, 'dist');
fs.mkdirSync(targetDir, { recursive: true });

const isWin = os.platform() === 'win32';
const binName = isWin ? 'agentsandbox.exe' : 'agentsandbox';

try {
  execSync('cargo build --release', { stdio: 'inherit' });
  const src = path.join(__dirname, 'target', 'release', binName);
  const dest = path.join(targetDir, binName);
  fs.copyFileSync(src, dest);
  fs.chmodSync(dest, 0o755);
} catch (err) {
  console.error('Failed to build agentsandbox binary', err);
  process.exit(1);
}
