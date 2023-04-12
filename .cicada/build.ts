import { Job, Pipeline, Secret } from "https://deno.land/x/cicada/lib.ts";

const secret1 = new Secret("TEST_VAR");

const muslJob = new Job({
  image: "rust:latest",
  steps: [
    "apt-get update && apt-get install -y musl-tools",
    "rustup target add x86_64-unknown-linux-musl",
    "cargo build -p cicada-cli --release --target x86_64-unknown-linux-musl",
  ],
});

const gnuJob = new Job({
  image: "rust:latest",
  steps: [
    {
      run: () => {
        const val = secret1.valueSync();
        console.log(`test - ${val}`);
      },
      secrets: [secret1],
    },
    "cargo build -p cicada-cli --release",
  ],
});

export default new Pipeline([muslJob, gnuJob]);
