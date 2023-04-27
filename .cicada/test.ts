import { Job, Pipeline } from "https://deno.land/x/cicada/mod.ts";

const testDeno = new Job({
  name: "Test Deno",
  image: "denoland/deno",
  steps: [
    "deno fmt --check",
    "deno lint",
    "deno check --all ./lib/mod.ts",
  ],
});

const testRust = new Job({
  name: "Test Rust",
  image: "rust",
  steps: [
    "apt-get update && apt-get install -y libssl-dev musl-tools protobuf-compiler",
    "rustup component add rustfmt clippy",
    "cargo fmt -- --check",
    "cargo clippy -- -D warnings",
    "cargo test",
    "cargo clippy --all-features -- -D warnings",
    "cargo test --all-features",
  ],
});

const testShell = new Job({
  name: "Test Shell",
  image: "ubuntu",
  steps: [
    "apt-get update && apt-get install -y shellcheck",
    `for file in $(find . -type f -name "*.sh"); do
        echo "Running shellcheck on $file"
        shellcheck "$file"
    done`,
  ],
});

const cargoDeny = new Job({
  name: "Check Cargo Deny",
  image: "rust",
  steps: [
    "cargo install cargo-deny",
    "cargo deny check",
  ],
});

const typos = new Job({
  name: "Check Typos",
  image: "rust",
  steps: [
    "cargo install typos-cli",
    "typos",
  ],
});

const pipeline = new Pipeline([
  testDeno,
  testRust,
  testShell,
  cargoDeny,
  typos,
]);

export default pipeline;
