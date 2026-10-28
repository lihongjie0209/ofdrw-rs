'use strict';

const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');

const packageRoot = path.resolve(__dirname, '..');
const libraries = new Map([
  ['linux-x64', 'libofdrw_ffi.so'],
  ['linux-arm64', 'libofdrw_ffi.so'],
  ['darwin-x64', 'libofdrw_ffi.dylib'],
  ['darwin-arm64', 'libofdrw_ffi.dylib'],
  ['win32-x64', 'ofdrw_ffi.dll'],
  ['win32-arm64', 'ofdrw_ffi.dll'],
]);

function destination(platform) {
  const library = libraries.get(platform);
  if (!library) throw new Error(`unsupported Node.js platform: ${platform}`);
  return path.join(packageRoot, 'native', platform, library);
}

function digest(data) {
  return crypto.createHash('sha256').update(data).digest('hex');
}

function fileDigest(file) {
  const hash = crypto.createHash('sha256');
  const descriptor = fs.openSync(file, 'r');
  const chunk = Buffer.allocUnsafe(1024 * 1024);
  try {
    let length;
    while ((length = fs.readSync(descriptor, chunk, 0, chunk.length, null)) !== 0) {
      hash.update(chunk.subarray(0, length));
    }
    return hash.digest('hex');
  } finally {
    fs.closeSync(descriptor);
  }
}

function interfaceDigest() {
  const wrapper = fs.readFileSync(path.join(packageRoot, 'index.js'), 'utf8');
  const signatures = [...wrapper.matchAll(/library\.func\('([^']+)'\)/g)]
    .map((match) => match[1])
    .sort();
  if (signatures.length === 0) throw new Error('Node.js wrapper has no native function declarations');
  return digest(signatures.join('\n'));
}

function manifest(platform) {
  return path.join(packageRoot, 'native', platform, 'manifest.json');
}

function main() {
  const [command, platform, source] = process.argv.slice(2);
  if (command === 'stage') {
    if (!source) throw new Error('usage: native-package.js stage PLATFORM SOURCE');
    if (platform !== `${process.platform}-${process.arch}`) {
      throw new Error(`requested ${platform}, but runner is ${process.platform}-${process.arch}`);
    }
    const nativeSource = path.resolve(source);
    if (!fs.statSync(nativeSource).isFile()) throw new Error(`native library is not a file: ${nativeSource}`);
    const previousOverride = process.env.OFDRW_LIB_PATH;
    process.env.OFDRW_LIB_PATH = nativeSource;
    try {
      const { Document } = require('../index.js');
      const document = Document.create();
      document.close();
    } finally {
      if (previousOverride === undefined) delete process.env.OFDRW_LIB_PATH;
      else process.env.OFDRW_LIB_PATH = previousOverride;
    }
    const target = destination(platform);
    fs.mkdirSync(path.dirname(target), { recursive: true });
    fs.copyFileSync(nativeSource, target);
    fs.writeFileSync(manifest(platform), JSON.stringify({
      interface_sha256: interfaceDigest(),
      library_sha256: fileDigest(target),
    }, null, 2) + '\n');
    return;
  }
  if (command === 'verify') {
    const currentInterface = interfaceDigest();
    const invalid = [...libraries.keys()].filter((name) => {
      try {
        const native = destination(name);
        if (fs.statSync(native).size === 0) return true;
        const staged = JSON.parse(fs.readFileSync(manifest(name), 'utf8'));
        return staged.interface_sha256 !== currentInterface
          || staged.library_sha256 !== fileDigest(native);
      } catch {
        return true;
      }
    });
    if (invalid.length) throw new Error(`native libraries missing or incompatible for: ${invalid.join(', ')}`);
    return;
  }
  throw new Error('usage: native-package.js stage PLATFORM SOURCE | verify');
}

main();
