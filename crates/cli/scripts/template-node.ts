import { Job, Pipeline } from "https://deno.land/x/cicada/mod.ts";

const build = new Job({
  name: "Node Build",
  image: "node",
  steps: [
    {
      name: "Install Dependencies",
      run: "npm install",
      cacheDirectories: ["node_modules"],
    },
    {
      name: "Run build",
      run: "npm run build",
      cacheDirectories: ["node_modules"],
    },
  ],
});

export default new Pipeline([build]);
