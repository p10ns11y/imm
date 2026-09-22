import { fold } from "./unicode.js";

export function slugify(input) {
  return fold(input).replaceAll(" ", "-");
}
