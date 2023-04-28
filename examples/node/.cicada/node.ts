import { Image } from "https://deno.land/x/cicada/experimental/image.ts";

// # Adjust NODE_VERSION as desired
// ARG NODE_VERSION=<%= nodeVersion %>
// FROM node:${NODE_VERSION}-slim as base

// LABEL fly_launch_runtime="<%= runtime %>"

// # <%= runtime %> app lives here
// WORKDIR /app

// # Set production environment
// ENV NODE_ENV=production
// <% if (yarn && yarnVersion != yarnClassic) { -%>

// ARG YARN_VERSION=<%= yarnVersion %>
// RUN npm install -g yarn@$YARN_VERSION --force
// <% } else if (pnpm) { -%>

// ARG PNPM_VERSION=<%= pnpmVersion %>
// RUN npm install -g pnpm@$PNPM_VERSION
// <% } %>

// # Throw-away build stage to reduce size of final image
// FROM base as build

// # Install packages needed to build node modules
// RUN apt-get update -qq && \
//     apt-get install -y <%= python %> pkg-config build-essential <%
//     if (prisma) { %>openssl <% } %>

// # Install node modules
// COPY --link <%= packageFiles.join(' ') %> ./
// RUN <%= packager %> install<% if (devDependencies) { %> --production=false<% } %>

// <% if (prisma) { -%>
// # Generate Prisma Client
// COPY --link prisma .
// RUN npx prisma generate

// <% } -%>
// # Copy application code
// COPY --link . .

// <% if (build) { -%>
// # Build application
// RUN <%= packager %> run build

// <% } -%>
// <% if (devDependencies && !nestjs) { -%>
// # Remove development dependencies
// <% if (yarn) { -%>
// RUN yarn install --production=true
// <% } else { -%>
// RUN npm prune --production
// <% } -%>

// <% } -%>

// # Final stage for app image
// FROM base

// # Copy built application
// COPY --from=build /app /app

// <% if (false && !options.root) /* needs more testing */ { -%>
// # Run as a non-root user for security
// RUN addgroup --system --gid 1001 nodejs && \
//     useradd <%= user %> --gid nodejs --home /app --shell /bin/bash
// USER <%= user %>:nodejs

// <% } -%>
// <% if (prisma) { -%>
// # Entrypoint prepares the database.
// ENTRYPOINT [ "/app/docker-entrypoint" ]

// <% } -%>
// # Start the server by default, this can be overwritten at runtime
// EXPOSE <%= port %>
// <% if (nuxtjs) { -%>
// ENV HOST=0
// <% } -%>
// CMD <%- JSON.stringify(startCommand, null, 1).replaceAll(/\n\s*/g, " ") %>

const nodeVersion = "18.16.0";

class Builder {
  // Parsed package.json file contents.
  // deno-lint-ignore no-explicit-any
  #pj: any;

  // which packager is used (npm, pnpm, yarn)
  #packager: "npm" | "pnpm" | "yarn";

  constructor() {
    this.#pj = this.packageJson;
    this.#packager = this.packager;
  }

  get packageJson() {
    return JSON.parse(Deno.readTextFileSync("package.json"));
  }

  // Does this application use remix?
  get remix() {
    return !!(this.#pj.dependencies.remix ||
      this.#pj.dependencies["@remix-run/node"]);
  }

  // Does this application use prisma?
  get prisma() {
    return !!(this.#pj.dependencies["@prisma/client"] ||
      this.#pj.devDependencies?.prisma);
  }

  // Does this application use next.js?
  get nextjs() {
    return !!this.#pj.dependencies.next;
  }

  // Does this application use nuxt.js?
  get nuxtjs() {
    return !!this.#pj.dependencies.nuxt;
  }

  // Does this application use gatsby?
  get gatsby() {
    return !!this.#pj.dependencies.gatsby;
  }

  // Does this application use nest?
  get nestjs() {
    return !!this.#pj.dependencies["@nestjs/core"];
  }

  get packageFiles() {
    const result = ["package.json"];

    for (const file of ["package-lock.json", "pnpm-lock.yaml", "yarn.lock"]) {
      try {
        Deno.statSync(file);
        result.push(file);
      } catch (_e) {
        // ignore
      }
    }

    return result;
  }

  get packager() {
    if (this.#packager !== undefined) return this.#packager;

    const packageFiles = this.packageFiles;

    if (packageFiles.includes("yarn.lock")) {
      this.#packager = "yarn";
    } else if (packageFiles.includes("pnpm-lock.yaml")) {
      this.#packager = "pnpm";
    } else {
      this.#packager = "npm";
    }

    return this.#packager;
  }

  get runtime() {
    let runtime = "Node.js";

    if (this.remix) runtime = "Remix";
    if (this.nextjs) runtime = "Next.js";
    if (this.nuxtjs) runtime = "Nuxt.js";
    if (this.nestjs) runtime = "NestJS";
    if (this.gatsby) runtime = "Gatsby";

    if (this.prisma) runtime += "/Prisma";

    return runtime;
  }

  get startCommand(): string[] {
    if (this.gatsby) {
      return ["npx", "gatsby", "serve", "-H", "0.0.0.0"];
    } else if (
      this.runtime === "Node.js" && this.#pj.scripts?.start?.includes("fastify")
    ) {
      let start = this.#pj.scripts.start;
      if (!start.includes("-a") && !start.includes("--address")) {
        start = start.replace("start", "start --address 0.0.0.0");
      }

      start = start.split(" ");
      start.unshift("npx");
      return start;
    } else {
      return [this.packager, "run", "start"];
    }
  }

  get port() {
    let port = 3000;

    if (this.gatsby) port = 8080;
    if (this.remix) port = 8080;

    return port;
  }
}

const builder = new Builder();

const image = new Image({
  name: "node",
  image: `node:${nodeVersion}-slim`,
  env: {
    NODE_ENV: "production",
  },
  steps: [
    `${builder.packager} install`,
    `${builder.packager} run build`,
  ],
  exposedPorts: [builder.port],
  cmd: builder.startCommand,
});

export default image;
