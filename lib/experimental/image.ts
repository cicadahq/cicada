import { CommonImageOptions } from "https://deno.land/x/cicada/mod.ts";

/**
 * An image that is built by Cicada
 */
export interface ImageOptions extends CommonImageOptions {
  /**
   * The name of the image. This will be used as the name of the docker image.
   */
  name: string;

  /**
   * Entry point to run when the container starts. This can be a string or an array of strings.
   * @example
   * // Will run `/bin/bash -c` as the base command for the container
   * `["/bin/bash", "-c"]`
   */
  entrypoint?: string[] | string;

  /**
   * The command to run when the container starts. This can be a string or an array of strings.
   *
   * @example
   * // Will run `echo Hello World` as the command for the container as an argument to the entrypoint
   * "echo Hello World"
   */
  cmd?: string[] | string;

  /**
   * The port that the container exposes. This can be a number or a string.
   *
   * @example
   * // Ports 8080 and 8081 will be exposed
   * [8080, "8081"]
   */
  exposedPorts?: (number | string)[];

  /**
   * The user that the container should run as.
   *
   * @example
   * "node"
   */
  user?: string;

  /**
   * The signal to send to the container to stop it.
   *
   * @example
   * "SIGINT"
   * "SIGTERM"
   */
  stopSignal?: string;
}

/**
 * Defines an image that is built
 */
export class Image {
  type = "image" as const;

  /**
   * @deprecated Do not use. The _uuid property is unstable and should be considered an internal implementation detail.
   */
  readonly _uuid = crypto.randomUUID();

  /**
   * Creates a new Image instance.
   * @param options - The options for the image.
   */
  constructor(public options: ImageOptions) {}
}
