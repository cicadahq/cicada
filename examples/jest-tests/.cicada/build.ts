import { Job, Pipeline } from "https://deno.land/x/cicada/mod.ts";

// Build a job for each Node.js version
const jobs: Job[] = ["14", "16", "18"].map((nodeVersion) =>
  new Job({
    name: `Test on Node.js ${nodeVersion}`,
    image: `node:${nodeVersion}`,
    cacheDirectories: ["node_modules"],
    steps: [
      // Run shell commands to install dependencies and run tests
      {
        name: "Install dependencies",
        run: "npm install",
      },
      {
        name: "Run tests",
        run: "npx jest",
      },
    ],
  })
);

export default new Pipeline(jobs);
