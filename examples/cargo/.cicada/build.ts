import { Job, Pipeline } from "https://deno.land/x/cicada/mod.ts";

const jobs: Job[] = ["bionic", "focal", "jammy"].map((
  ubuntuRelease,
) =>
  new Job({
    image: `ubuntu:${ubuntuRelease}`,
    env: {
      ARCH: "x86_64",
      TEST_ENV: "test",
    },
    steps: [
      {
        name: "Install dependencies",
        run: `
          # apt-get update
          # apt-get install -y curl unzip cargo
        `,
      },
      "echo 'Hello world' > test.txt",
      () => console.log("Hello world"),
      "mv test.txt /workspace",
    ],
  })
);

export default new Pipeline(jobs);

// function orchestrator(workflows) {
//   // Create dag based on dependencies
//   // Run each workflow in parallel till it hits a dependency and run the next one

//   const out = await workflows.run();
//   await Promise.all([
//     focal.run({
//       mount: {
//         "/app": out.output["bin"],
//       },
//     }),
//     jammy.run(),
//   ]);
// },

// export default new Workflow([
//   new Container({
//     name: "test",
//     docker: "alpine",
//     run: (
//       @Integration(Shell)
//       shell: Shell,
//       @Integration(Curl)
//       curl: Curl,
//     ) => ([
//       () => shell.run("apk add curl"),
//       () => shell.run("curl https://dagger.io"),
//     ])
//   }),
// ])

/*
// initialize Dagger client
connect(async (client: Client) => {
  // Set Node versions against which to test and build
  const nodeVersions = ["12", "14", "16"]

  // get reference to the local project
  const source = await client.host().directory(".", { exclude: ["node_modules/"] })

  // for each Node version
  for (const nodeVersion of nodeVersions) {
    // mount cloned repository into Node image
    const runner = client
      .container().from(`node:${nodeVersion}`)
      .withMountedDirectory("/src", source)
      .withWorkdir("/src")
      .withExec(["npm", "install"])

    // run tests
    await runner.withExec(["npm", "test", "--", "--watchAll=false"]).exitCode()

    // build application using specified Node version
    // write the build output to the host
    await runner
      .withExec(["npm", "run", "build"])
      .directory("build/")
      .export(`./build-node-${nodeVersion}`)
  }
}, {LogOutput: process.stdout})
*/

// FROM node:${version}
// COPY --exclude Cargo.lock --from context-name . .
// RUN npm install
// RUN npm test -- --watchAll=false
// RUN npm run build

// export default new Workflow([
//   ["12", "14", "16"].map((nodeVersion) => new Container({
//     name: `node-${nodeVersion}`,
//     docker: `node:${nodeVersion}`,
//     // ignore: ["Cargo.lock"],
//     // directory: "/src",
//     run: (
//       @Integration(Npm)
//       npm: Npm,
//       @Workspace()
//       workspace: Workspace,
//     ) => {
//       npm.install();
//       npm.run("test", {
//         args: ["--watchAll=false"],
//       });
//       npm.run("build");
//       workspace.copyFile("build", `build-node-${nodeVersion}`);
//     }
//   }))
// ], {
//   // github: "dagger/dagger",
//   cron: "0 0 * * *",
//   onPush: true,
// })
