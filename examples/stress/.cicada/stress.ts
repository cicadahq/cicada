import { Job, Pipeline, Secret } from "https://deno.land/x/cicada/mod.ts";

const job0 = new Job({
  image: "ubuntu:22.04",
  steps: [
    "echo Hello, world!",
  ],
});

const job1 = new Job({
  name: "My Second Job",
  image: "ubuntu:22.04",
  cacheDirectories: [
    {
      path: "node_modules",
      sharing: "locked",
    },
    "dist",
    {
      path: "build",
      sharing: "private",
    },
    {
      path: "build",
    },
    {
      path: "build",
      sharing: "shared",
    },
  ],
  env: {
    "MY_ENV": "my value",
  },
  dependsOn: [job0],
  onFail: "ignore",
  workingDirectory: "src",
  steps: [
    {
      name: "Print a message",
      run: "echo Hello, world!",
    },
    {
      name: "Run a js function",
      run: () => {
        console.log("Hello from js");
      },
      ignoreCache: true,
      cacheDirectories: [
        {
          path: "node_modules",
          sharing: "locked",
        },
        "dist",
        {
          path: "build",
          sharing: "private",
        },
        {
          path: "build",
        },
        {
          path: "build",
          sharing: "shared",
        },
      ],
      env: {
        "MY_ENV": "my value",
      },
      secrets: [
        new Secret("MY_SECRET"),
      ],
      workingDirectory: "src",
    },
  ],
});

export default new Pipeline([
  job0,
  job1,
]);
