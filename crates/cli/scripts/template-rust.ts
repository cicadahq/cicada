import {
  CacheDirectories,
  Job,
  Pipeline,
} from "https://deno.land/x/cicada/mod.ts";

const cacheDirectories: CacheDirectories = [
  "target",
  "/usr/local/cargo/registry",
  "/usr/local/cargo/git",
];

const lintJob = new Job({
  name: "Build",
  image: "rust:latest",
  steps: [
    {
      run: "cargo fmt --check",
      cacheDirectories,
    },
    {
      run: "cargo clippy",
      cacheDirectories,
    },
  ],
});

const buildJob = new Job({
  name: "Build",
  image: "rust:latest",
  steps: [
    {
      run: "cargo build",
      cacheDirectories,
    },
  ],
});

export default new Pipeline(
  [lintJob, buildJob],
  {
    on: {
      pullRequest: ["main"],
      push: ["main"],
    },
  },
);
