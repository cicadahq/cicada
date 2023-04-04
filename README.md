# Cicada

> [Cicada](https://cicada.build): Write CI/CD pipelines in TypeScript

### How to use
```typescript
import {Pipeline, Job} as cicada from "https://deno.land/x/cicada/lib.ts"
```

### Pipelines, Jobs, and Steps
* **Pipeline**: A pipeline is the highest level concept in Cicada. It is a TypeScript file like `build.ts`, `deploy,ts`, or `run_tests.ts`. A pipeline is triggered when an event occurs in your repository such as a new commit or a pull request being opened. A pipeline is an array of jobs that  are executed sequentially or in parallel.
* **Jobs**: A job is an array of steps executed on the same container/runner. 
* **Steps**: A step is either a shell script or Deno script executed in the job's container

![image](https://user-images.githubusercontent.com/4949076/229649044-b385b525-946e-4a86-a66d-773547770105.png)

**Example**  
You have a Pipeline called "Tests". It has jobs called "Cypress" and "Playwright" that execute in separate containers. Each job has multiple steps for cloning your code, installing the testing framework, and executing the tests

### Support
👉 **Docs**: https://deno.land/x/cicada@v0.1.2/lib.ts  
👉 **Discord**: https://discord.gg/g2PRPm4u4Y


### Roadmap
- [x] Local runner execution
- [ ] Cloud runner execution
- [ ] Parallel job execution

