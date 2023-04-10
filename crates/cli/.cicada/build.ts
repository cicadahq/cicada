import { Job, Pipeline } from "https://deno.land/x/cicada/lib.ts";

const job = new Job({
  image: "rust:latest",
  steps: [
    "apt-get update && apt-get install -y musl-tools",
    "rustup target add x86_64-unknown-linux-musl",
    "cargo build --release --target x86_64-unknown-linux-musl",
  ],
});

export default new Pipeline([job]);
