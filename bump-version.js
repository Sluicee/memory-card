import fs from 'fs';
import path from 'path';

const releaseType = process.argv[2];

if (!releaseType) {
  console.error('Usage: npm run bump <patch|minor|major|version_number>');
  process.exit(1);
}

function bumpSemver(current, type) {
  const parts = current.split('.');
  if (parts.length !== 3) {
    throw new Error(`Invalid version format: ${current}`);
  }
  let [major, minor, patch] = parts.map(Number);
  if (isNaN(major) || isNaN(minor) || isNaN(patch)) {
    throw new Error(`Non-numeric version components in: ${current}`);
  }

  if (type === 'major') {
    major += 1;
    minor = 0;
    patch = 0;
  } else if (type === 'minor') {
    minor += 1;
    patch = 0;
  } else if (type === 'patch') {
    patch += 1;
  } else {
    // If it's a specific version string, validate it
    if (!/^\d+\.\d+\.\d+$/.test(type)) {
      throw new Error(`Invalid release type or version string: ${type}`);
    }
    return type;
  }
  return `${major}.${minor}.${patch}`;
}

function writePreservingLineEndings(filePath, content, originalContent) {
  const hasCrlf = originalContent.includes('\r\n');
  let output = content;
  if (hasCrlf) {
    output = output.replace(/\r?\n/g, '\r\n');
  } else {
    output = output.replace(/\r\n/g, '\n');
  }
  fs.writeFileSync(filePath, output, 'utf8');
}

try {
  // 1. package.json
  const pkgPath = path.resolve('package.json');
  const pkgOriginal = fs.readFileSync(pkgPath, 'utf8');
  const pkg = JSON.parse(pkgOriginal);
  const currentVersion = pkg.version;
  const newVersion = bumpSemver(currentVersion, releaseType);
  
  console.log(`Bumping version: ${currentVersion} -> ${newVersion}`);

  pkg.version = newVersion;
  const pkgNewContent = JSON.stringify(pkg, null, 2) + '\n';
  writePreservingLineEndings(pkgPath, pkgNewContent, pkgOriginal);
  console.log('✓ Updated package.json');

  // 2. package-lock.json
  const pkgLockPath = path.resolve('package-lock.json');
  if (fs.existsSync(pkgLockPath)) {
    const pkgLockOriginal = fs.readFileSync(pkgLockPath, 'utf8');
    const pkgLock = JSON.parse(pkgLockOriginal);
    pkgLock.version = newVersion;
    if (pkgLock.packages && pkgLock.packages['']) {
      pkgLock.packages[''].version = newVersion;
    }
    const pkgLockNewContent = JSON.stringify(pkgLock, null, 2) + '\n';
    writePreservingLineEndings(pkgLockPath, pkgLockNewContent, pkgLockOriginal);
    console.log('✓ Updated package-lock.json');
  }

  // 3. src-tauri/tauri.conf.json
  const tauriConfPath = path.resolve('src-tauri/tauri.conf.json');
  if (fs.existsSync(tauriConfPath)) {
    const tauriConfOriginal = fs.readFileSync(tauriConfPath, 'utf8');
    const tauriConf = JSON.parse(tauriConfOriginal);
    tauriConf.version = newVersion;
    const tauriConfNewContent = JSON.stringify(tauriConf, null, 2) + '\n';
    writePreservingLineEndings(tauriConfPath, tauriConfNewContent, tauriConfOriginal);
    console.log('✓ Updated src-tauri/tauri.conf.json');
  }

  // 4. src-tauri/Cargo.toml
  const cargoTomlPath = path.resolve('src-tauri/Cargo.toml');
  if (fs.existsSync(cargoTomlPath)) {
    const cargoTomlOriginal = fs.readFileSync(cargoTomlPath, 'utf8');
    let cargoToml = cargoTomlOriginal;
    const cargoTomlRegex = /^version\s*=\s*"[^"]+"/m;
    if (cargoTomlRegex.test(cargoToml)) {
      cargoToml = cargoToml.replace(cargoTomlRegex, `version = "${newVersion}"`);
      writePreservingLineEndings(cargoTomlPath, cargoToml, cargoTomlOriginal);
      console.log('✓ Updated src-tauri/Cargo.toml');
    } else {
      console.warn('⚠️ Could not find version key in src-tauri/Cargo.toml');
    }
  }

  // 5. src-tauri/Cargo.lock
  const cargoLockPath = path.resolve('src-tauri/Cargo.lock');
  if (fs.existsSync(cargoLockPath)) {
    const cargoLockOriginal = fs.readFileSync(cargoLockPath, 'utf8');
    let cargoLock = cargoLockOriginal;
    const cargoLockRegex = /(\[\[package\]\]\r?\nname\s*=\s*"memory-card"\r?\nversion\s*=\s*")[^"]+(")/;
    if (cargoLockRegex.test(cargoLock)) {
      cargoLock = cargoLock.replace(cargoLockRegex, `$1${newVersion}$2`);
      writePreservingLineEndings(cargoLockPath, cargoLock, cargoLockOriginal);
      console.log('✓ Updated src-tauri/Cargo.lock');
    } else {
      console.warn('⚠️ Could not find package "memory-card" in src-tauri/Cargo.lock');
    }
  }

  console.log(`Successfully bumped to v${newVersion}!`);
} catch (error) {
  console.error('Error bumping version:', error.message);
  process.exit(1);
}
