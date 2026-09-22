import { auditUse, extractUse, installSource, overwriteUse } from "./ops.mjs";

export const SERVER_INFO = { name: "imm", version: "0.1.0" };

export const TOOLS = [
  {
    name: "imm_install",
    description: "Install the whole source of one direct dependency. Does not install its dependency tree.",
    inputSchema: {
      type: "object",
      properties: {
        name: { type: "string" },
        version: { type: "string" },
      },
      required: ["name", "version"],
    },
  },
  {
    name: "imm_extract",
    description: "Extract one used function from a package. Returns the hash and waits for an audit. Does not install the package.",
    inputSchema: {
      type: "object",
      properties: {
        name: { type: "string" },
        version: { type: "string" },
        fn: { type: "string" },
      },
      required: ["name", "version", "fn"],
    },
  },
  {
    name: "imm_audit",
    description: "Audit one extract by id. The id looks like name@version#fn.",
    inputSchema: {
      type: "object",
      properties: {
        id: { type: "string" },
        by: { type: "string" },
        at: { type: "string" },
      },
      required: ["id", "by", "at"],
    },
  },
  {
    name: "imm_overwrite",
    description: "Overwrite an extract with agent-modified source and store the diff against the original hash.",
    inputSchema: {
      type: "object",
      properties: {
        id: { type: "string" },
        text: { type: "string" },
      },
      required: ["id", "text"],
    },
  },
];

function toolText(result) {
  return {
    content: [{ type: "text", text: JSON.stringify(result) }],
    isError: result.ok === false,
  };
}

export function callTool(name, args, ctx) {
  if (name === "imm_install") {
    return toolText(installSource({ ...args, registryDir: ctx.registryDir, stateDir: ctx.stateDir }));
  }
  if (name === "imm_extract") {
    return toolText(extractUse({ ...args, registryDir: ctx.registryDir, stateDir: ctx.stateDir, now: ctx.now }));
  }
  if (name === "imm_audit") {
    return toolText(auditUse({ ...args, stateDir: ctx.stateDir }));
  }
  if (name === "imm_overwrite") {
    return toolText(overwriteUse({ ...args, stateDir: ctx.stateDir }));
  }
  return toolText({ ok: false, reasons: ["unknown-tool"] });
}

export function handleMessage(message, ctx) {
  if (!message || message.jsonrpc !== "2.0") return null;
  if (message.method === "notifications/initialized") return null;
  const id = message.id ?? null;
  if (message.method === "initialize") {
    return {
      jsonrpc: "2.0",
      id,
      result: {
        protocolVersion: "2024-11-05",
        capabilities: { tools: {} },
        serverInfo: SERVER_INFO,
      },
    };
  }
  if (message.method === "tools/list") {
    return { jsonrpc: "2.0", id, result: { tools: TOOLS } };
  }
  if (message.method === "tools/call") {
    const result = callTool(message.params?.name, message.params?.arguments ?? {}, ctx);
    return { jsonrpc: "2.0", id, result };
  }
  if (id === null) return null;
  return { jsonrpc: "2.0", id, error: { code: -32601, message: "Method not found" } };
}

export function encodeMessage(message) {
  const body = JSON.stringify(message);
  return `Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`;
}

export function serve(input, output, ctx) {
  return new Promise((resolve) => {
    let buf = Buffer.alloc(0);
    const onData = (chunk) => {
      buf = Buffer.concat([buf, chunk]);
      while (buf.length > 0) {
        const taken = takeMessage(buf);
        if (!taken || taken.error) break;
        if (!taken.message) break;
        buf = Buffer.from(taken.rest);
        const response = handleMessage(taken.message, ctx);
        if (response) output.write(encodeMessage(response));
      }
    };
    input.on("data", onData);
    input.on("end", () => resolve());
  });
}

export function takeMessage(buffer) {
  const text = buffer.toString("utf8");
  const headerEnd = text.indexOf("\r\n\r\n");
  if (headerEnd === -1) return null;
  const header = text.slice(0, headerEnd);
  const match = header.match(/Content-Length:\s*(\d+)/i);
  if (!match) return { error: "missing-length" };
  const length = Number(match[1]);
  const start = Buffer.byteLength(text.slice(0, headerEnd + 4));
  if (buffer.length < start + length) return null;
  const body = buffer.subarray(start, start + length).toString("utf8");
  return { message: JSON.parse(body), rest: buffer.subarray(start + length) };
}
