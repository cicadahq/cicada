import {
  CacheDirectoryOptions,
  FilePath,
  Pipeline,
  Shell,
  Step,
  StepFn,
  Trigger,
} from "https://deno.land/x/cicada/mod.ts";

const mapCache = (
  cache: FilePath | CacheDirectoryOptions,
): CacheDirectoryOptions => {
  if (typeof cache === "string") {
    return {
      path: cache,
    };
  }

  return cache;
};

const modulePath = Deno.args[0];
const outPath = Deno.args[1];

Deno.stat(new URL(modulePath)).catch(() => {
  console.error(
    "%cError:",
    "color: red; font-weight: bold;",
    "Could not find module at",
    modulePath,
  );
  Deno.exit(1);
});

const module = await import(modulePath);
const pipeline: Pipeline = module.default;

type TriggerOn = {
  type: "branches";
  branches: string[];
} | {
  type: "all";
};

type SerializedTrigger =
  | {
    type: "options";
    push: TriggerOn;
    pullRequest: TriggerOn;
  }
  | {
    type: "denoFunction";
  };

type SerializedPipeline = {
  jobs: SerializedJob[];
  on: SerializedTrigger;
};

type SerializedRun =
  | {
    type: "command";
    command: string;
  }
  | {
    type: "denoFunction";
  };

type SerializedShell = {
  type: "bash";
} | {
  type: "sh";
} | {
  type: "args";
  args: string[];
};

type SerializedJob = {
  uuid: string;
  image: string;
  steps: SerializedStep[];
  name: string | undefined;
  env: Record<string, string> | undefined;
  cacheDirectories: CacheDirectoryOptions[] | undefined;
  workingDirectory: string | undefined;
  dependsOn: string[] | undefined;
  onFail: "ignore" | "stop" | undefined;
};

type SerializedStep = {
  run: SerializedRun;
  name: string | undefined;
  cacheDirectories: CacheDirectoryOptions[] | undefined;
  ignoreCache: boolean | undefined;
  env: Record<string, string> | undefined;
  secrets: string[] | undefined;
  workingDirectory: string | undefined;
  shell: SerializedShell | undefined;
};

const serializeShell = (shell: Shell): SerializedShell => {
  if (shell === "bash") {
    return {
      type: "bash",
    };
  } else if (shell === "sh") {
    return {
      type: "sh",
    };
  } else {
    return {
      type: "args",
      args: shell.args,
    };
  }
};

const serializeRun = (run: string | StepFn): SerializedRun => {
  if (typeof run === "string") {
    return {
      type: "command",
      command: run,
    } as const;
  } else if (typeof run === "function") {
    return {
      type: "denoFunction",
    } as const;
  } else {
    throw new Error("Invalid run type");
  }
};

const serializeStep = (step: Step): SerializedStep => {
  if (typeof step === "string" || typeof step === "function") {
    return {
      run: serializeRun(step),
      name: undefined,
      cacheDirectories: undefined,
      ignoreCache: undefined,
      env: undefined,
      secrets: undefined,
      workingDirectory: undefined,
      shell: undefined,
    };
  } else {
    return {
      name: step.name,
      run: serializeRun(step.run),
      env: step.env,
      secrets: step.secrets?.map((s) => s.name),
      cacheDirectories: step.cacheDirectories?.map(mapCache),
      ignoreCache: step.ignoreCache,
      workingDirectory: step.workingDirectory,
      shell: step.shell ? serializeShell(step.shell) : undefined,
    };
  }
};

const serializeTrigger = (trigger?: Trigger): SerializedTrigger => {
  let push: TriggerOn = { type: "all" };
  let pullRequest: TriggerOn = { type: "all" };
  if (trigger) {
    if (trigger.push && trigger.push !== "all") {
      push = {
        type: "branches",
        branches: trigger.push,
      };
    }

    if (trigger.pullRequest && trigger.pullRequest !== "all") {
      pullRequest = {
        type: "branches",
        branches: trigger.pullRequest,
      };
    }
  }

  return {
    type: "options",
    push,
    pullRequest,
  };
};

const serializePipeline = (pipeline: Pipeline): SerializedPipeline => {
  const jobs: SerializedJob[] = [];

  for (const job of pipeline.jobs) {
    jobs.push({
      uuid: job._uuid,
      image: job.options.image,
      steps: job.options.steps.map(serializeStep),
      name: job.options.name,
      env: job.options.env,
      cacheDirectories: job.options.cacheDirectories?.map(mapCache),
      workingDirectory: job.options.workingDirectory,
      dependsOn: job.options.dependsOn?.map((j) => j._uuid),
      onFail: job.options.onFail,
    });
  }

  return {
    jobs,
    // name: pipeline.options?.name ?? undefined,
    on: serializeTrigger(pipeline.options?.on),
  };
};

const serializedPipeline = serializePipeline(pipeline);

await Deno.writeTextFile(outPath, JSON.stringify(serializedPipeline, null, 2));
