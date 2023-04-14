import { Job, Pipeline } from "https://deno.land/x/cicada/lib.ts";

const job = new Job({
  name: "Simple job",
  image: "ubuntu:latest",
  steps: [
    {
      name: "Print a message",
      run: "echo Hello, world!",
    },
    "ls -al /usr/local/bin",
    "pwd",
    {
      name: "Run a js function",
      run: () => {
        console.log("Hello from js");
      },
    },
  ],
});

export default new Pipeline([job], {
  on: {
    push: ["main"],
  },
});
