import { Pipeline } from "https://deno.land/x/cicada/mod.ts";
import { Image } from "https://deno.land/x/cicada/expiremental/image.ts";

const moduleFile = Deno.env.get("CICADA_PIPELINE_FILE");
if (!moduleFile) {
  console.error(
    "%cError:",
    "color: red; font-weight: bold;",
    "CICADA_PIPELINE_FILE not set",
  );
  Deno.exit(1);
}

const module = await import(moduleFile);
const workflow: Pipeline | Image = module.default;
const workflowNum = parseInt(Deno.args[0], 10);
const stepNum = parseInt(Deno.args[1], 10);
const job = workflow instanceof Pipeline
  ? workflow.jobs[workflowNum]
  : workflow;

const step = job.options.steps[stepNum];

let name: string;
if (typeof step === "object") {
  if (step.name) {
    name = step.name;
  } else {
    name = `Step ${stepNum}`;
  }
} else {
  name = `Step ${stepNum}`;
}

console.log(`Running ${job.options.image} ${name}`);

let script: string | undefined;
let fn: (() => void | Promise<void> | number | Promise<number>) | undefined;
if (typeof step === "object") {
  if (typeof step.run === "string") {
    script = step.run;
  } else if (typeof step.run === "function") {
    fn = step.run;
  }
} else if (typeof step === "string") {
  script = step;
} else if (typeof step === "function") {
  fn = step;
}

let status: number | undefined;
if (script) {
  try {
    status = (await new Deno.Command("sh", {
      args: ["-c", script],
    }).spawn().status).code;
  } catch (e) {
    if (e instanceof Error) {
      console.error(
        "%cError:",
        "color: red; font-weight: bold;",
        e.message,
      );

      if (e.stack) {
        console.error(e.stack);
      }
    } else {
      console.error(
        "%cError:",
        "color: red; font-weight: bold;",
        e,
      );
    }
    status = 1;
  }
} else if (fn) {
  try {
    const out = await fn();
    if (typeof out === "number") {
      status = out;
    }
  } catch (e) {
    if (e instanceof Error) {
      console.error(
        "%cError:",
        "color: red; font-weight: bold;",
        e.message,
      );

      if (e.stack) {
        console.error(e.stack);
      }
    } else {
      console.error(
        "%cError:",
        "color: red; font-weight: bold;",
        e,
      );
    }

    status = 1;
  }
}

if (status) {
  Deno.exit(status);
}
