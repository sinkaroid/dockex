#!/usr/bin/env node

const { spawnSync } = require("child_process");
const fs = require("fs");
const path = require("path");

function getBinaryPath() {
  const isWindows = process.platform === "win32";
  const binaryName = isWindows ? "dockex.exe" : "dockex";

  // 1. Check local cargo release build (development mode)
  const localTarget = path.join(__dirname, "..", "target", "release", binaryName);
  if (fs.existsSync(localTarget)) {
    return localTarget;
  }

  // 2. Check vendor/dist binary inside package root
  const rootBin = path.join(__dirname, binaryName);
  if (fs.existsSync(rootBin)) {
    return rootBin;
  }

  // 3. Check optionalDependency platform package
  const platformPkg = `@sinkaroid/dockex-${process.platform}-${process.arch}`;
  try {
    const pkgPath = require.resolve(`${platformPkg}/bin/${binaryName}`);
    if (fs.existsSync(pkgPath)) {
      if (!isWindows) {
        try { fs.chmodSync(pkgPath, 0o755); } catch {}
      }
      return pkgPath;
    }
  } catch {
    try {
      const pkgJsonPath = require.resolve(`${platformPkg}/package.json`);
      const directBin = path.join(path.dirname(pkgJsonPath), "bin", binaryName);
      if (fs.existsSync(directBin)) {
        if (!isWindows) {
          try { fs.chmodSync(directBin, 0o755); } catch {}
        }
        return directBin;
      }
    } catch {
      // Platform package not installed
    }
  }

  console.error(`\x1b[31m❌ Error: dockex binary not found for ${process.platform}-${process.arch}.\x1b[0m`);
  console.error(`Please install with prebuilt binaries or build locally via 'cargo build --release'.`);
  process.exit(1);
}

// Prevent Node launcher from dying on SIGINT so the native binary prints the final detachment message
process.on("SIGINT", () => {});

const binPath = getBinaryPath();
const result = spawnSync(binPath, process.argv.slice(2), {
  stdio: "inherit",
});

if (result.error) {
  console.error(`\x1b[31mFailed to start dockex: ${result.error.message}\x1b[0m`);
  process.exit(1);
}

process.exit(result.status ?? 0);

