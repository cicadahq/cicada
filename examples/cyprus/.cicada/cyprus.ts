import { Job, Pipeline } from "https://deno.land/x/cicada/mod.ts";

const job = new Job({
  name: "Cypress POC",
  image: "cypress/included",
  steps: [
    {
      name: "install packages",
      run: "npm install",
    },
    {
      name: "verify cypress",
      run: "npx cypress info",
    },
  ],
});

export default new Pipeline([job]);
