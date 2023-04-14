const fs = require("fs");
const os = require("os");
const path = require("path");

// This feature was added to give external code a way to modify the binary
// path without modifying the code itself. Do not remove this because
// external code relies on this.
const CICADA_BINARY_PATH = process.env.CICADA_BINARY_PATH;

const PACKAGE_BY_PLATFORM = {
  "darwin arm64": "cicada-aarch64-apple-darwin",
  "darwin x64": "cicada-x86_64-apple-darwin",
  "win32 x64": "cicada-x86_64-pc-windows-msvc",
  "linux x64": "cicada-x86_64-unknown-linux-musl",
};

function pkgAndSubpathForCurrentPlatform() {
  const platform = os.platform();
  const arch = os.arch();

  const pkg = PACKAGE_BY_PLATFORM[`${platform} ${arch}`];
  if (pkg === null) {
    throw new Error(`Unsupported platform: ${platform} ${arch}`);
  }

  return pkg;
}

function generateBinPath() {
  if (CICADA_BINARY_PATH) {
    if (!fs.existsSync(CICADA_BINARY_PATH)) {
      console.warn(
        `[cicada] Ignoring bad configuration: CICADA_BINARY_PATH=${CICADA_BINARY_PATH}`,
      );
    } else {
      return CICADA_BINARY_PATH;
    }
  }

  const pkg = pkgAndSubpathForCurrentPlatform();
  const binDir = path.join(path.resolve(__dirname), "bin");

  // Create the "bin" directory if it does not exist
  if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir, { recursive: true });
  }

  let binPath = path.join(binDir, pkg);
  if (os.platform() === "win32") {
    binPath = path.join(binDir, `${pkg}.exe`);
  }

  return binPath;
}

module.exports = {
  CICADA_BINARY_PATH,
  pkgAndSubpathForCurrentPlatform,
  generateBinPath,
};
