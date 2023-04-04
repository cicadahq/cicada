/**
 * A file path represented as a string.
 */
export type FilePath = string;

/**
 * Options for a cached directory
 */
export type CacheDirectoryOptions = {
  path: FilePath;
  /**
   * Directories default to `shared`
   *
   * - `shared` - can be used concurrently by multiple writers
   * - `private` - creates a new cache if there are multiple writers
   * - `locked` - pauses the second writer until the first one releases the cache
   */
  sharing?: "shared" | "private" | "locked";
};

/**
 * A directory to cache, which can be a single file path or an array of file paths.
 */
export type CacheDirectories = (
  | FilePath
  | CacheDirectoryOptions
)[];

/**
 * A step function that can return void or a number and can be synchronous or asynchronous.
 */
export type StepFn = () => void | Promise<void> | number | Promise<number>;

/**
 * A step in the pipeline, which can be an object with a name and a run property,
 * a step function, or a string command.
 */
export type Step =
  | {
    /**
     * The command to run as a string or a step function.
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
  | StepFn
  | string;

/**
 * The options for a job, including the name, image, environment variables,
 * cache, and steps.
 */
export type JobOptions = {
  /**
   * The docker image to use for this job.
   *
   * @example "node", "node:18", "node:18-alpine"
   */
  image: string;
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
};

/**
 * Represents a job in the pipeline with its options.
 */
export class Job {
  /**
   * Do not used. The _uuid property is unstable and should be considered an internal implementation detail.
   * @internal
   */
  readonly _uuid: string;

  /**
   * Creates a new Job instance.
   * @param options - The options for the job.
   */
  constructor(public options: JobOptions) {
    this._uuid = crypto.randomUUID();
  }
}

/**
 * Represents a pipeline with an array of jobs.
 */
export class Pipeline {
  /**
   * Creates a new Pipeline instance.
   * @param jobs - The jobs to include in the pipeline.
   */
  constructor(public jobs: Job[]) {}
}

function inJob() {
  return Boolean(Deno.env.get("CICADA_JOB"));
}

export class Secret {
  /**
   * Creates a new Secret instance.
   * @param name - The name of the secret.
   */
  constructor(public name: string) {
    if (inJob()) {
      try {
        Deno.statSync(`/run/secrets/${name}`);
      } catch (_e) {
        throw new Error(
          `Secret \`${name}\` is not available in this job, make sure it is specified in the job options.`,
        );
      }
    }
  }

  /**
   * Get a secret value from the secrets directory. The secret is only available during the job if it is specified in the job options.
   *
   * @returns The secret value
   */
  async value(): Promise<string> {
    if (!inJob()) {
      throw new Error("Secrets are only available during a job.");
    }
    const secretPath = `/run/secrets/${this.name}`;
    const secret = await Deno.readTextFile(secretPath);
    return secret;
  }

  /**
   * Get a secret value from the secrets directory. The secret is only available during the job if it is specified in the job options.
   * This is a synchronous version of `getSecret`.
   *
   * @returns The secret value
   */
  valueSync(): string {
    if (!inJob()) {
      throw new Error("Secrets are only available during a job.");
    }
    const secretPath = `/run/secrets/${this.name}`;
    const secret = Deno.readTextFileSync(secretPath);
    return secret;
  }
}
