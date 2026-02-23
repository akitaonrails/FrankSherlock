/** Extract the last segment of a path (file or folder name). */
export function basename(path: string): string {
  return path.split("/").pop() || path;
}

/** Extract a human-readable message from an unknown error value. */
export function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
