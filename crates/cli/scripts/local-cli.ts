import {
  CacheDirectoryOptions,
  FilePath,
  Pipeline,
  Step,
  StepFn,
Trigger,
} from "https://deno.land/x/cicada/lib.ts";

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

type SerializedTrigger = 
| {
  type: "options";
  push: string[];
  pullRequest: string[];
}
| {
  type: "denoFunction";

}

type SerializedPipeline = {
  jobs: SerializedJob[];
  options: {
    name?: string;
    on: SerializedTrigger
  };
}

type SerializedRun =
  | {
    type: "command";
    command: string;
  }
  | {
    type: "denoFunction";
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
    };
  }
};

const serializeTrigger = (trigger?: Trigger): SerializedTrigger => {

  return {
    type: "options",
    push: trigger?.push ?? [],
    pullRequest: trigger?.pullRequest ?? [],
  }
}


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
    options: {
      name: pipeline.options?.name ?? undefined,
      on: serializeTrigger(pipeline.options?.on)
    },

  }
};

const serializedPipeline = serializePipeline(pipeline);

await Deno.writeTextFile(outPath, JSON.stringify(serializedPipeline, null, 2));
