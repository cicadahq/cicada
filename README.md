# [Cicada](https://cicada.build)

**Write CI/CD pipelines in TypeScript**

## Pipelines, Jobs, and Steps

- **Pipeline**: A pipeline is the highest level concept in Cicada. It is a
  TypeScript file like `build.ts`, `deploy,ts`, or `run_tests.ts`. A pipeline is
  triggered when an event occurs in your repository such as a new commit or a
  pull request being opened. A pipeline is an array of jobs that are executed
  sequentially or in parallel.
- **Jobs**: A job is an array of steps executed on the same container/runner.
- **Steps**: A step is either a shell script or Deno script executed in the
  job's container

![image](https://user-images.githubusercontent.com/4949076/229649044-b385b525-946e-4a86-a66d-773547770105.png)

#### **Example**

You have a Pipeline called "Tests". It has jobs called "Cypress" and
"Playwright" that execute in separate containers. Each job has multiple steps
for cloning your code, installing the testing framework, and executing the tests

## Getting started

### 1. Dependencies

- Docker (at least version 23.0) -
  [Installation Guide](https://docs.docker.com/desktop/)

#### MacOS Quickstart

```bash
brew install --cask docker
```

### 2. Download the Cicada CLI

Use this script to download the latest release of Cicada:

```bash
curl -fSsL https://raw.githubusercontent.com/cicadahq/cicada/main/download.sh | sh
```

### 3. Create a pipeline

Go to the project you want to make a pipeline for and run:

```bash
cicada init
```

### 4. Run the pipeline

```bash
cicada run .cicada/<pipeline-name>.ts
```

### 5. Set up autocomplete for .cicada files (Optional)

Install the Deno extension for VSCode:

```bash
code --install-extension denoland.vscode-deno
```

Add the following to your `.vscode/settings.json`

```json
{
  "deno.enablePaths": [".cicada"]
}
```

## Support

👉 **Docs**: https://deno.land/x/cicada/mod.ts

👉 **Discord**: https://discord.gg/g2PRPm4u4Y

## Roadmap

- Add direct integrations with buildkit
- Integrate deeper with deno
