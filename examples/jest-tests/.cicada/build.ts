import { Job, Pipeline } from "https://deno.land/x/cicada/lib.ts";

// Build a job for each Node.js version
const jobs: Job[] = ["14", "16", "18"].map((nodeVersion) =>
  new Job({
    name: `Test on Node.js ${nodeVersion}`,
    image: `node:${nodeVersion}`,
    cacheDirectories: [".npm"],
    steps: [
      // Run shell commands to install dependencies and run tests
      {
        name: "Install dependencies",
        run: "npm set cache .npm && npm install",
      },
      // {
      //   name: "Run tests",
      //   run: "npm run test",
      // },
      {
        name: "js",
        run: () => {
          console.log("js");
        },
      },
      // Use native TypeScript to send a message to Slack
      // {
      //   name: "Send message to Slack",
      //   secrets: [
      //     "SLACK_TOKEN",
      //     "SLACK_TEST_CHANNEL",
      //   ],
      //   run: async () => {
      //     await fetch("https://slack.com/api/chat.postMessage", {
      //       method: "POST",
      //       headers: {
      //         "Content-Type": "application/json",
      //         Authorization: `Bearer ${await getSecret("SLACK_TOKEN")}`,
      //       },
      //       body: JSON.stringify({
      //         channel: await getSecret("SLACK_TEST_CHANNEL"),
      //         text: `Tests passed on Node.js ${nodeVersion}`,
      //       }),
      //     });
      //   },
      // },
    ],
  })
);

export default new Pipeline(jobs);
