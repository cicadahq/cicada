import {
  CacheDirectoryOptions,
  FilePath,
  Image,
  Job,
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

let pipeline: Pipeline | Image;

if (module.default instanceof Pipeline || module.default instanceof Image) {
  pipeline = module.default;
} else {
  console.error(
    "%cError:",
    "color: red; font-weight: bold;",
    "Module does not export a Pipeline or Image",
  );
  Deno.exit(1);
}

type SerializedTriggerOn = {
  type: "branches";
  branches: string[];
} | {
  type: "all";
};

type SerializedTrigger =
  | {
    type: "options";
    push?: SerializedTriggerOn;
    pullRequest?: SerializedTriggerOn;
  }
  | {
    type: "denoFunction";
  };

type SerializedPipeline = {
  type: "pipeline";
  jobs: SerializedJob[];
  on?: SerializedTrigger;
};

type SerializedRun =
  | {
    type: "command";
    command: string;
  }
  | {
    type: "args";
    args: string[];
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

const serializeRun = (run: string | string[] | StepFn): SerializedRun => {
  if (typeof run === "string") {
    return {
      type: "command",
      command: run,
    } as const;
  } else if (Array.isArray(run)) {
    return {
      type: "args",
      args: run,
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

const serializeTriggerOn = (
  triggerOn: string[] | "all",
): SerializedTriggerOn => {
  if (triggerOn === "all") {
    return {
      type: "all",
    };
  } else {
    return {
      type: "branches",
      branches: triggerOn,
    };
  }
};

const serializeTrigger = (trigger: Trigger): SerializedTrigger => {
  return {
    type: "options",
    push: trigger.push ? serializeTriggerOn(trigger.push) : undefined,
    pullRequest: trigger.pullRequest
      ? serializeTriggerOn(trigger.pullRequest)
      : undefined,
  };
};

const serializeJob = (job: Job): SerializedJob => {
  return {
    uuid: job._uuid,
    image: job.options.image,
    steps: job.options.steps.map(serializeStep),
    name: job.options.name,
    env: job.options.env,
    cacheDirectories: job.options.cacheDirectories?.map(mapCache),
    workingDirectory: job.options.workingDirectory,
    dependsOn: job.options.dependsOn?.map((j) => j._uuid),
    onFail: job.options.onFail,
  };
};

const serializePipeline = (pipeline: Pipeline): SerializedPipeline => {
  const jobs: SerializedJob[] = [];

  for (const job of pipeline.jobs) {
    jobs.push(serializeJob(job));
  }

  return {
    type: "pipeline",
    jobs,
    on: pipeline.options?.on
      ? serializeTrigger(pipeline.options?.on)
      : undefined,
  };
};

let object;
if (pipeline instanceof Pipeline) {
  object = serializePipeline(pipeline);
} else {
  const job = new Job({
    ...pipeline.options,
  });

  object = {
    type: "image",
    ...serializeJob(job),
  };
}

await Deno.writeTextFile(outPath, JSON.stringify(object, null, 2));
