export type FilePath = string;
export type Cache = FilePath | FilePath[];

export type StepFn = () => void | Promise<void> | number | Promise<number>;

export type Step =
  | {
    name: string;
    run: string | StepFn;
  }
  | StepFn
  | string;

export type JobOptions = {
  name?: string;
  image: string;
  env?: Record<string, string>;
  cache?: Cache; 
  steps: Step[];
};

export class Job {
  options: JobOptions;

  constructor(options: JobOptions) {
    this.options = options;
  }
}

export class Pipeline {
  jobs: Job[];

  constructor(jobs: Job[]) {
    this.jobs = jobs;
  }
}
