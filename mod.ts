import { resolve } from "https://deno.land/std@0.182.0/path/mod.ts";
import { DockerImages } from "./types/dockerImages.ts";

/**
 * A file path represented as a string.
 */
export type FilePath = string;

/**
 * Options for a cached directory
 */
export interface CacheDirectoryOptions {
  path: FilePath;
  /**
   * Directories default to `shared`
   *
   * - `shared` - can be used concurrently by multiple writers
   * - `private` - creates a new cache if there are multiple writers
   * - `locked` - pauses the second writer until the first one releases the cache
   */
  sharing?: "shared" | "private" | "locked";
}

/**
 * A directory to cache, which can be a single file path or an array of file paths.
 */
export type CacheDirectories = (FilePath | CacheDirectoryOptions)[];

/**
 * A step function that can return void or a number and can be synchronous or asynchronous.
 */
export type StepFn = () => void | Promise<void> | number | Promise<number>;

/**
 * A step with all available options
 */
export interface StepOptions {
  /**
   * The command to run as a string or a {@link StepFn step function}.
   */
  run: string | StepFn;

  /**
   * The name of the step.
   */
  name?: string;

  /**
   * Cache directories, these will be mounted as docker volumes.
   *
   * If the path is absolute, it will be mounted as is, otherwise it will be mounted relative to the project root.
   * This will mount for all steps in the job.
   */
  cacheDirectories?: CacheDirectories;

  /**
   * Disable caching for this step, this will cause the step to run every time, it may cause subsequent steps to run as well.
   * @default false
   */
  ignoreCache?: boolean;

  /**
   * Environment variables to set for this step.
   */
  env?: Record<string, string>;

  /**
   * Secrets to expose for this step. They are accessed with `getSecret` or via the `/var/run/secrets` directory.
   */
  secrets?: Secret[];

  /**
   * Specify the working directory where this job should run
   */
  workingDirectory?: FilePath;
}

/**
 * A step in the pipeline, which can be an object with a name and a run property,
 * a step function, or a string command.
 */
export type Step =
  | StepOptions
  | StepFn
  | string;

/**
 * The options for a job, including the name, image, environment variables,
 * cache, and steps.
 */
export interface JobOptions {
  /**
   * The docker image to use for this job.
   *
   * @example "node", "node:18", "node:18-alpine"
   */
  image: DockerImages;

  /**
   * A list of steps to run in the job.
   */
  steps: Step[];

  /**
   * The name of the job, this will be used in the logs.
   */
  name?: string;

  /**
   * Environment variables to set for this job.
   */
  env?: Record<string, string>;

  /**
   * Cache directories, these will be mounted as docker volumes.
   *
   * If the path is absolute, it will be mounted as is, otherwise it will be mounted relative to the project root.
   * This will mount for all steps in the job.
   */
  cacheDirectories?: CacheDirectories;

  /**
   * Specify the working directory where this job should run
   */
  workingDirectory?: FilePath;

  /**
   * Require these jobs to run before the current job can be executed.
   */
  dependsOn?: Job[];

  /**
   * What to do if the job fails.
   * - `ignore` - ignore the failure and continue the pipeline
   * - `stop` - stop the pipeline
   */
  onFail?: "ignore" | "stop";
}

/**
 * Represents a job in the pipeline with its options.
 */
export class Job {
  /**
   * @deprecated Do not use. The _uuid property is unstable and should be considered an internal implementation detail.
   */
  readonly _uuid = crypto.randomUUID();

  /**
   * Creates a new Job instance.
   * @param options - The options for the job.
   */
  constructor(public options: JobOptions) {}
}

/**
 * A git branch represented as a string.
 */
export type Branch = string;

/**
 * The declarative options for a trigger.
 */
export interface TriggerOptions {
  push?: Branch[];
  pullRequest?: Branch[];
}

/**
 * A trigger function that returns a boolean value indicating whether the pipeline should run.
 */
// export type TriggerFn = () => boolean | Promise<boolean>;

/**
 * The trigger for a pipeline
 */
export type Trigger = TriggerOptions; //TriggerFn | TriggerOptions;

/**
 * The options for a pipeline, including the name and the conditions under which the pipeline should run.
 */
export interface PipelineOptions {
  /**
   * The name of the pipeline
   */
  name?: string;
  /**
   * The trigger declares the conditions under which the pipeline should run.
   */
  on: Trigger;
}

/**
 * Represents a pipeline containing an array of jobs.
 */
export class Pipeline {
  /**
   * Creates a new Pipeline instance.
   * @param jobs - An array of jobs to include in the pipeline.
   * @param options - The options for the pipeline.
   */
  constructor(public jobs: Job[], public options?: PipelineOptions) {}
}

export class Secret {
  static readonly #isInJob = Deno.env.has("CICADA_JOB");
  static readonly #secretsDir = "/run/secrets";
  #path = "";

  /**
   * Creates a new Secret instance.
   *
   * @param name - The name of the secret.
   */
  constructor(public name: string) {
    if (!Secret.#isInJob) return;
    this.#path = resolve(Secret.#secretsDir, name);
  }

  /**
   * A check that the secret file exists to avoid a cryptic error message.
   */
  #assertFileExists = () => {
    try {
      Deno.statSync(this.#path);
    } catch (_) {
      throw new Error(
        `Secret \`${this.name}\` is not available in this job, make sure it is specified in the job options.`,
      );
    }
  };

  /**
   * Get a secret value from the secrets directory. The secret is only available during the job if it is specified in the job options.
   * This is an asynchronous version of {@linkcode valueSync()}.
   *
   * @returns The secret value
   */
  value(): Promise<string> {
    if (!Secret.#isInJob) {
      throw new Error("Secrets are only available during a job.");
    }

    this.#assertFileExists();

    return Deno.readTextFile(this.#path);
  }

  /**
   * Get a secret value from the secrets directory. The secret is only available during the job if it is specified in the job options.
   * This is a synchronous version of {@linkcode value()}.
   *
   * @returns The secret value
   */
  valueSync(): string {
    if (!Secret.#isInJob) {
      throw new Error("Secrets are only available during a job.");
    }

    this.#assertFileExists();

    return Deno.readTextFileSync(this.#path);
  }
}
