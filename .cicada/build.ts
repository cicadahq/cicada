import { Job, Pipeline } from "https://deno.land/x/cicada/lib.ts";

const env = {
  RUST_VERSION: "1.68.2",
  CARGO_HOME: "/usr/local/cargo",
  RUSTUP_HOME: "/usr/local/rustup",
}

const muslJob = new Job({
  name: "Build musl",
  image: "rust:latest",
  env,
  steps: [
    "apt-get update && apt-get install -y musl-tools protobuf-compiler",
    'PATH="/usr/local/cargo/bin:$PATH" rustup target add x86_64-unknown-linux-musl',
    'PATH="/usr/local/cargo/bin:$PATH" cargo build -p cicada-cli --release --target x86_64-unknown-linux-musl',
  ],
});

const gnuJob = new Job({
  name: "Build gnu",
  image: "rust:latest",
  env,
  steps: [
    'PATH="/usr/local/cargo/bin:$PATH" cargo build -p cicada-cli --release',
  ],
});

export default new Pipeline([muslJob, gnuJob]);
