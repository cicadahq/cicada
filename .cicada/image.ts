import { Image } from "https://deno.land/x/cicada/mod.ts";

const image = new Image({
  name: "my-app",
  image: "ubuntu:20.04",
  steps: [
    "apt-get update && apt-get install -y gcc",
    `cat <<EOF > hello-world.c
#include <stdio.h>

int main() {
  printf("Hello, World!");
  return 0;
}
EOF`,
    "gcc hello-world.c -o hello-world",
  ],
});

export default image;
