import { extname, join } from "https://deno.land/std@0.182.0/path/mod.ts";

for (const examples of Deno.readDirSync(".")) {
  if (!examples.isDirectory) continue;
  const cicadaDir = join(examples.name, ".cicada");

  for (const pipeline of Deno.readDirSync(cicadaDir)) {
    if (extname(pipeline.name) !== ".ts") continue;

    const path = join(cicadaDir, pipeline.name);
    console.log(path);

    const child = new Deno.Command("cargo", {
      args: [
        "run",
        "-p",
        "cicada-cli",
        "--",
        "run",
        "--cicada-dockerfile",
        "../docker/bin.Dockerfile",
        path,
      ],
    }).spawn();

    const status = await child.status;
    if (!status.success) {
      console.error(`Failed to run ${path}`);
      Deno.exit(1);
    }
  }
}
