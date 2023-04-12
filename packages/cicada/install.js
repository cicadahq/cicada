const fs = require("fs");
const zlib = require("zlib");
const https = require("https");
const {
  CICADA_BINARY_PATH,
  pkgAndSubpathForCurrentPlatform,
  generateBinPath,
} = require("./node-platform");
const CICADA_VERSION = require("./package.json").version;

function fetch(url) {
  return new Promise((resolve, reject) => {
    https
      .get(url, (res) => {
        if (
          (res.statusCode === 301 || res.statusCode === 302) &&
          res.headers.location
        ) {
          return fetch(res.headers.location).then(resolve, reject);
        }
        if (res.statusCode !== 200) {
          return reject(new Error(`Server responded with ${res.statusCode}`));
        }
        let chunks = [];
        res.on("data", (chunk) => chunks.push(chunk));
        res.on("end", () => resolve(Buffer.concat(chunks)));
      })
      .on("error", reject);
  });
}

function extractFileFromTarGzip(buffer, subpath) {
  try {
    buffer = zlib.unzipSync(buffer);
  } catch (err) {
    throw new Error(
      `Invalid gzip data in archive: ${(err && err.message) || err}`
    );
  }
  let str = (i, n) =>
    String.fromCharCode(...buffer.subarray(i, i + n)).replace(/\0.*$/, "");
  let offset = 0;
  // subpath = `package/${subpath}`;
  while (offset < buffer.length) {
    let name = str(offset, 100);
    let size = parseInt(str(offset + 124, 12), 8);
    offset += 512;
    if (!isNaN(size)) {
      if (name === subpath) return buffer.subarray(offset, offset + size);
      offset += (size + 511) & ~511;
    }
  }
  throw new Error(`Could not find ${JSON.stringify(subpath)} in archive`);
}

async function install() {
  if (CICADA_BINARY_PATH) {
    return;
  }

  const pkg = pkgAndSubpathForCurrentPlatform();

  console.error(`[cicada] Fetching package "${pkg}" from GitHub`);

  const binPath = generateBinPath();
  const url = `https://github.com/cicadahq/cicada/releases/download/v${CICADA_VERSION}/${pkg}.tar.gz`;

  try {
    const tarGzip = await fetch(url);
    fs.writeFileSync(binPath, extractFileFromTarGzip(tarGzip, "cicada"));
    fs.chmodSync(binPath, 0o755);
  } catch (e) {
    console.error(
      `[cicada] Failed to download ${JSON.stringify(url)}: ${
        (e && e.message) || e
      }`
    );
    throw e;
  }
}

install().then(() => {
  console.log("[cicada] CLI successfully installed");
  process.exit(0);
});
