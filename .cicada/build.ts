import { Job, Pipeline } from "https://deno.land/x/cicada/lib.ts";

const muslJob = new Job({
  name: "Build musl",
  image: "rust:latest",
  steps: [
    "apt-get update && apt-get install -y musl-tools",
    "rustup target add x86_64-unknown-linux-musl",
    "cargo build -p cicada-cli --release --target x86_64-unknown-linux-musl",
  ],
});

// const gnuJob = new Job({
//   name: "Build gnu",
//   image: "rust:latest",
//   steps: [
//     "cargo build -p cicada-cli --release",
//   ],
// });

export default new Pipeline([muslJob,]);
