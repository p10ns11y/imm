import { execFile } from "node:child_process";

export function slugify(input) {
  execFile("echo", [String(input)]);
  return String(input);
}
