/**
 * Finds an available port starting from 3000, writes a temporary Tauri
 * config override file, and launches `tauri dev` with --config pointing
 * at that file.  Both devUrl and beforeDevCommand are overridden so that
 * Next.js and the Tauri WebView use the same dynamic port.
 */

import { createServer } from "net";
import { spawn } from "child_process";
import { writeFileSync, unlinkSync } from "fs";
import { join, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const CONFIG_PATH = join(__dirname, "..", "src-tauri", "tauri.dev.conf.json");
const START_PORT = 3000;

async function findAvailablePort(port) {
  const tryBind = (host) =>
    new Promise((resolve, reject) => {
      const server = createServer();
      server.unref();
      server.on("error", reject);
      server.listen({ port, host, exclusive: true }, () => {
        server.close(resolve);
      });
    });

  try {
    // Try both IPv6 and IPv4
    await tryBind("::");
    await tryBind("0.0.0.0");
    await tryBind("127.0.0.1");
    await tryBind("localhost");
    return port;
  } catch (err) {
    if (err.code === "EADDRINUSE" || err.code === "EACCES") {
      return findAvailablePort(port + 1);
    }
    throw err;
  }
}

const port = await findAvailablePort(START_PORT);

if (port !== START_PORT) {
  console.log(`Port ${START_PORT} is in use — using port ${port} instead.`);
} else {
  console.log(`Starting dev server on port ${port}.`);
}

// Write a temporary config override that Tauri will merge (RFC 7396)
const override = {
  build: {
    devUrl: `http://localhost:${port}`,
    beforeDevCommand: `npx next dev --turbopack --port ${port}`,
  },
};

writeFileSync(CONFIG_PATH, JSON.stringify(override, null, 2));

function cleanup() {
  try {
    unlinkSync(CONFIG_PATH);
  } catch {
    // ignore if already removed
  }
}

process.on("exit", cleanup);
process.on("SIGINT", () => {
  cleanup();
  process.exit(130);
});
process.on("SIGTERM", () => {
  cleanup();
  process.exit(143);
});

const child = spawn("npx", ["tauri", "dev", "--config", CONFIG_PATH], {
  stdio: "inherit",
});

child.on("exit", (code) => process.exit(code ?? 1));
