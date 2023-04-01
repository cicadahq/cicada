import { Octokit } from "https://cdn.skypack.dev/@octokit/core@3?dts";
import { getSecretSync } from "../lib.ts";

/**
 * Get the GitHub repository from the `GITHUB_REPOSITORY` environment variable
 */
export function getGitHubRepository() {
  const repoEnvVar = Deno.env.get("GITHUB_REPOSITORY");
  if (!repoEnvVar) {
    throw new Error("GITHUB_REPOSITORY environment variable not set");
  }

  const [owner, repo] = repoEnvVar.split("/");
  return {
    owner,
    repo,
  };
}

/**
 * Make sure that the step exposes `GH_TOKEN` as a secret
 */
export function getGitHubToken() {
  const tokenEnvVar = getSecretSync("GH_TOKEN");
  if (!tokenEnvVar) {
    throw new Error(
      "Github token is not set, ensure the step exposes `GH_TOKEN`",
    );
  }
  return tokenEnvVar;
}

/**
 * Create an Octokit instance with the GitHub token
 *
 * Ensure that the `GH_TOKEN` secret is exposed in the step
 */
export function createOctoKit() {
  return new Octokit({
    auth: getGitHubToken(),
  });
}
