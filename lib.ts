/**
 * A file path represented as a string.
 */
export type FilePath = string;

/**
 * A directory to cache, which can be a single file path or an array of file paths.
 */
export type CacheDir = FilePath | FilePath[];

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
     * The name of the step.
     */
    name: string;
    /**
     * The command to run as a string or a step function.
     */
    run: string | StepFn;
    /**
     * Cache directories, these will be mounted as docker volumes.
     *
     * If the path is absolute, it will be mounted as is, otherwise it will be mounted relative to the project root.
     * This will mount for all steps in the job.
     */
    cacheDir?: CacheDir;
    /**
     * Disable caching for this step, this will cause the step to run every time, it may cause subsequent steps to run as well.
     */
    cache?: boolean;
    /**
     * Environment variables to set for this step.
     */
    env?: Record<string, string>;
    /**
     * Secrets to expose for this step. They are accessed with `getSecret` or via the `/var/run/secrets` directory.
     */
    secrets?: string[];
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
  cacheDir?: CacheDir;
  /**
   * Specify the working directory where this job should run
   */
  workingDirectory?: FilePath;
  /**
   * Require these jobs to run before the current job can be executed.
   */
  dependsOn?: Job[];
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

/**
 * Get a secret value from the secrets directory. The secret is only available during the job if it is specified in the job options.
 *
 * @param name The name of the secret
 * @returns The secret value
 */
export async function getSecret(name: string): Promise<string> {
  const secretPath = `/run/secrets/${name}`;
  const secret = await Deno.readTextFile(secretPath);
  return secret;
}

/**
 * Get a secret value from the secrets directory. The secret is only available during the job if it is specified in the job options.
 * This is a synchronous version of `getSecret`.
 *
 * @param name The name of the secret
 * @returns The secret value
 */
export function getSecretSync(name: string): string {
  const secretPath = `/run/secrets/${name}`;
  const secret = Deno.readTextFileSync(secretPath);
  return secret;
}
