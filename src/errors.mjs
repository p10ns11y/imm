export class ImmError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "ImmError";
    this.code = code;
  }
}
