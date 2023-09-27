> [!IMPORTANT]
> Cicada is archived and will no longer be maintained. Thank you for your contributions and support.

# Cicada

> **[Cicada](https://cicada.build)**: Write CI/CD pipelines in TypeScript, test
> them locally

## Quickstart

Test a pipeline on your local device in < 2 minutes

```bash
# Install Cicada
npm install -g @cicadahq/cicada 

# Set up Cicada in a project
cd path/to/my/project
cicada init

# Test your pipeline locally
cicada run <my-pipeline>
```

Deploy your pipeline to our cloud so it runs on every PR/commit:

1. Sign up at [cicada.build/dashboard](https://cicada.build/dashboard)
2. Link your repository using our GitHub integration
3. Push your pipeline to GitHub

## Example

```typescript
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
      },
    },
  ],
});

export default new Pipeline([job]);
```

## Terminology

- **Pipeline**: Pipelines are TypeScript files like `build.ts`, `deploy.ts`, or
  `run_tests.ts`. They are checked into your repository and run when triggered
  by an event in your repository, or when triggered manually, or at a defined
  schedule. A pipeline takes one parameter: an array of jobs.
- **Jobs**: A job is a lightweight container that executes code. It takes one
  parameter: an array of steps.
- **Steps**: A step is either a shell script or Deno/TypeScript script that
  executes in its parent job’s container

## 3rd party modules

Check out [cicadahq/modules](https://github.com/cicadahq/modules)

## Support

👉 **Docs**: [cicada.build/docs](https://cicada.build/docs)

👉 **Typescript API**: [deno.land/x/cicada](https://deno.land/x/cicada/mod.ts)

👉 **Discord**: [cicada.build/discord](https://discord.gg/g2PRPm4u4Y)

## Enterprise

Need self-hosted runners, advanced security and compliance, custom integrations,
or something else? We can help!

Please email [brendan@fig.io](mailto:brendan@fig.io)
