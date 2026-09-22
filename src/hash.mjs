import { createHash } from "node:crypto";

export function sha256(content) {
  const data = Buffer.isBuffer(content) ? content : Buffer.from(content, "utf8");
  return createHash("sha256").update(data).digest("hex");
}
