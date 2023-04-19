import { Job, Pipeline, Secret } from "https://deno.land/x/cicada/lib.ts";

const useSecret = new Job({
  name: "Use Secret",
  image: "ubuntu:22.04",
  steps: [
    {
      name: "Print a message",
      run: "echo $MY_SECRET",
      secrets: [
        new Secret("MY_SECRET"),
      ],
    },
  ],
});

export default new Pipeline([useSecret]);
