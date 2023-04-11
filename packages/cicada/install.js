const fs = require("fs");
const os = require("os");
const path = require("path");
const axios = require("axios");
const tar = require("tar");
const extract = require("extract-zip");
const packageJson = require("./package.json");

async function downloadFile(url, outputPath) {
  const response = await axios({
    method: "get",
    url,
    responseType: "stream",
  });

  const writer = fs.createWriteStream(outputPath);
  response.data.pipe(writer);

  return new Promise((resolve, reject) => {
    writer.on("finish", resolve);
    writer.on("error", reject);
  });
}

async function installBinary() {
  const platform = os.platform();
  const arch = os.arch();

  let binaryFileName;
  if (platform === "darwin" && arch === "arm64") {
    binaryFileName = `cicada-aarch64-apple-darwin.tar.gz`;
  } else if (platform === "darwin" && arch === "x64") {
    binaryFileName = `cicada-x86_64-apple-darwin.tar.gz`;
  } else if (platform === "win32" && arch === "x64") {
    binaryFileName = `cicada-x86_64-pc-windows-msvc.zip`;
  } else if (platform === "linux" && arch === "x64") {
    binaryFileName = "cicada-x86_64-unknown-linux-musl.tar.gz";
  } else {
    console.error(`Unsupported platform or architecture: ${platform}-${arch}`);
    process.exit(1);
  }

  const binaryUrl =
    `https://github.com/cicadahq/cicada/releases/download/v${packageJson.version}/${binaryFileName}`;

  let binaryName = `cicada`;
  if (platform === "win32") {
    binaryName = `cicada.exe`;
  }

  const binDir = path.join(__dirname, "bin");
  const binaryPath = path.join(binDir, binaryName);

  if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir);
  }

  const tempDir = path.join(__dirname, "temp");
  const tempArchivePath = path.join(tempDir, binaryFileName);

  if (!fs.existsSync(tempDir)) {
    fs.mkdirSync(tempDir);
  }

  try {
    await downloadFile(binaryUrl, tempArchivePath);

    let extractedBinaryPath;
    if (binaryFileName.endsWith(".tar.gz")) {
      await tar.x({
        file: tempArchivePath,
        cwd: tempDir,
      });
      extractedBinaryPath = path.join(tempDir, binaryName);
    } else if (binaryFileName.endsWith(".zip")) {
      await extract(tempArchivePath, { dir: tempDir });
      extractedBinaryPath = path.join(tempDir, "out", binaryName);
    } else {
      throw new Error(`Unsupported archive format: ${binaryFileName}`);
    }

    if (fs.existsSync(extractedBinaryPath)) {
      fs.renameSync(extractedBinaryPath, binaryPath);
    } else {
      throw new Error("Could not find the binary in the extracted content.");
    }

    // Remove temporary files
    fs.unlinkSync(tempArchivePath);
    fs.rmdirSync(tempDir, { recursive: true });

    fs.chmodSync(binaryPath, 0o755);
    console.log(`Downloaded and installed Cicada for ${platform}-${arch}`);
  } catch (error) {
    console.error(`Error downloading and installing ${binaryName}:`, error);
    process.exit(1);
  }
}

installBinary();
