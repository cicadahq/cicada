import { Job, Pipeline } from "https://deno.land/x/cicada/mod.ts";

const job00 = new Job({
  image: "ubuntu:22.04",
  steps: [
    "echo '1'",
  ],
});

const job01 = new Job({
  image: "ubuntu:22.04",
  steps: [
    "echo '2'",
  ],
});

const job10 = new Job({
  image: "ubuntu:22.04",
  dependsOn: [job00, job01],
  steps: [
    "echo '3'",
  ],
});

const job20 = new Job({
  image: "ubuntu:22.04",
  dependsOn: [job10],
  steps: [
    "echo '4'",
  ],
});

const job30 = new Job({
  image: "ubuntu:22.04",
  dependsOn: [job01, job10],
  steps: [
    "echo '5'",
  ],
});

export default new Pipeline([job00, job01, job10, job20, job30]);
