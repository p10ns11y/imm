import { execFile } from "node:child_process";

export function run() {
  execFile("echo", ["no"]);
}
