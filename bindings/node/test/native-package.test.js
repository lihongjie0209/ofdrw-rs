'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const { execFileSync, spawnSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const platforms = [
  ['linux-x64', 'libofdrw_ffi.so'],
  ['linux-arm64', 'libofdrw_ffi.so'],
  ['darwin-x64', 'libofdrw_ffi.dylib'],
  ['darwin-arm64', 'libofdrw_ffi.dylib'],
  ['win32-x64', 'ofdrw_ffi.dll'],
  ['win32-arm64', 'ofdrw_ffi.dll'],
];

function sha256(value) {
  return crypto.createHash('sha256').update(value).digest('hex');
}

test('native package verification rejects stale interfaces and changed binaries', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'ofdrw-native-package-'));
  try {
    const script = path.join(root, 'scripts', 'native-package.js');
    fs.mkdirSync(path.dirname(script), { recursive: true });
    fs.copyFileSync(path.join(__dirname, '..', 'scripts', 'native-package.js'), script);
    const wrapper = path.join(root, 'index.js');
    fs.writeFileSync(wrapper, "library.func('int ofd_example(void)')\n");
    const interfaceHash = sha256('int ofd_example(void)');
    for (const [platform, library] of platforms) {
      const directory = path.join(root, 'native', platform);
      fs.mkdirSync(directory, { recursive: true });
      const bytes = Buffer.from(`library for ${platform}`);
      fs.writeFileSync(path.join(directory, library), bytes);
      fs.writeFileSync(path.join(directory, 'manifest.json'), JSON.stringify({
        interface_sha256: interfaceHash,
        library_sha256: sha256(bytes),
      }));
    }
    const verify = () => execFileSync(process.execPath, [script, 'verify'], { stdio: 'pipe' });
    assert.doesNotThrow(verify);

    fs.appendFileSync(path.join(root, 'native', 'linux-x64', 'libofdrw_ffi.so'), 'changed');
    assert.throws(verify, /linux-x64/);
    fs.writeFileSync(path.join(root, 'native', 'linux-x64', 'libofdrw_ffi.so'), 'library for linux-x64');
    fs.writeFileSync(wrapper, "library.func('int ofd_changed(void)')\n");
    assert.throws(verify, /native libraries missing or incompatible/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('explicit native library path does not fall back to another library', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'ofdrw-missing-native-'));
  try {
    const missing = path.join(root, 'missing-native-library');
    const result = spawnSync(process.execPath,
      ['-e', 'require(process.argv[1])', path.join(__dirname, '..')], {
        env: { ...process.env, OFDRW_LIB_PATH: missing },
        encoding: 'utf8',
      });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /cannot load OFDRW_LIB_PATH/);
    assert.match(result.stderr, /missing-native-library/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
