import { Job, Pipeline } from "https://deno.land/x/cicada/mod.ts";

const job = new Job({
  name: "My First Job",
  image: "ubuntu:22.04",
  steps: [
    {
      name: "Run bash",
      run: "echo hello from bash!",
    },
    {
      name: "Run deno/typescript",
      run: () => {
        console.log("Hello from deno typescript");
        console.log("I can read the local file system");
        console.log("e.g. here are all the directories in my project");
        console.log(Array.from(Deno.readDirSync("/app")));
        console.log("Or I can see my environment variables");
        console.log(Deno.env.toObject());
      },
    },
  ],
});

export default new Pipeline(
  [job],
  {
    on: {
      pullRequest: ["main"],
      push: ["main"],
    },
  },
);
