// Thin wrapper over the generated bindings: turns `{ status: "error" }`
// results into exceptions carrying the backend error kind and message.

import { commands, type AppError, type ErrorKind } from "./bindings";

export { commands };

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: AppError };

/** Error raised by a backend command, with a user-facing message. */
export class CommandError extends Error {
  readonly kind: ErrorKind;

  constructor(error: AppError) {
    super(error.message);
    this.name = "CommandError";
    this.kind = error.kind;
  }
}

/** Awaits a command result and returns its data or throws a `CommandError`. */
export async function call<T>(result: Promise<Result<T>>): Promise<T> {
  const r = await result;
  if (r.status === "error") throw new CommandError(r.error);
  return r.data;
}

/** Short title for an error kind. */
export function errorTitle(kind: ErrorKind | undefined): string {
  switch (kind) {
    case "invalid_file":
      return "Invalid file";
    case "geometry":
      return "Invalid geometry";
    case "solver":
      return "Analysis failed";
    case "mach":
      return "Mach number too high";
    case "invalid_input":
      return "Invalid input";
    case "not_found":
      return "Not found";
    case "project":
      return "Project file";
    case "io":
      return "File error";
    default:
      return "Error";
  }
}
