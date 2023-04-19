import {
  Job,
  Pipeline,
} from "https://raw.githubusercontent.com/cicadahq/cicada/main/mod.ts";

const joba = new Job({
  image: "docker",
  steps: [],
});

joba.options.dependsOn;

// Build a job for each Node.js version
const jobs: Job[] = ["14", "16", "18"].map((nodeVersion) =>
  new Job({
    image: `node:${nodeVersion}`,
    cacheDir: ".npm",
    steps: [
      // Run shell commands to install dependencies and run tests
      {
        name: "Install dependencies",
        run: "npm set cache .npm && npm install",
      },

      {
        name: "Run tests",
        run: "npm run test",
      },
      // Use native TypeScript to send a message to Slack
      // {
      //   name: "Send message to Slack",
      //   run: async () => {
      //     await fetch("https://slack.com/api/chat.postMessage", {
      //       method: "POST",
      //       headers: {
      //         "Content-Type": "application/json",
      //         Authorization: `Bearer ${getSecret("SLACK_TOKEN")}`,
      //       },
      //       body: JSON.stringify({
      //         channel: getSecret("SLACK_TEST_CHANNEL"),
      //         text: `Tests passed on Node.js ${nodeVersion}`,
      //       }),
      //     });
      //   },
      // },
    ],
  })
);

export default new Pipeline(jobs);
