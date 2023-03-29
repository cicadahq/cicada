/**
 * A file path represented as a string.
 */
export type FilePath = string;

/**
 * A cache, which can be a single file path or an array of file paths.
 */
export type Cache = FilePath | FilePath[];

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
  }
  | StepFn
  | string;

/**
 * The options for a job, including the name, image, environment variables,
 * cache, and steps.
 */
export type JobOptions = {
  /**
   * The name of the job.
   */
  name?: string;
  /**
   * The Docker image to use for the job.
   */
  image: string;
  /**
   * Environment variables for the job.
   */
  env?: Record<string, string>;
  /**
   * Cache for the job.
   */
  cache?: Cache;
  /**
   * The steps to execute in the job.
   */
  steps: Step[];
};

/**
 * Represents a job in the pipeline with its options.
 */
export class Job {
  /**
   * Creates a new Job instance.
   * @param options - The options for the job.
   */
  constructor(public options: JobOptions) {}
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
