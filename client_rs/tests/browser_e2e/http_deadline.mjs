// Run from any directory: node client_rs/tests/browser_e2e/http_deadline.mjs
// Uses the existing Playwright installation and wasm-bindgen CLI prerequisites.
import http from "node:http";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { chromium, firefox } from "playwright";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const temporary = await fs.mkdtemp(path.join(os.tmpdir(), "client-rs-http-deadline-"));
const sockets = new Set();
let server;
let browser;
function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, encoding: "utf8", timeout: 300_000 });
  if (result.error || result.status !== 0) {
    throw new Error(`${command} failed: ${result.error || result.stderr || result.stdout}`);
  }
  return result.stdout;
}
try {
  const build = run("cargo", [
    `+${process.env.CLIENT_RS_TOOLCHAIN || "stable"}`, "build", "-p", "atm0s-media-client-browser",
    "--example", "http_deadline_probe", "--target", "wasm32-unknown-unknown", "--message-format=json",
  ]);
  const artifact = build.split("\n").filter(Boolean).map(line => JSON.parse(line))
    .find(item => item.reason === "compiler-artifact" && item.target.name === "http_deadline_probe");
  const wasm = artifact?.filenames.find(filename => filename.endsWith(".wasm"));
  if (!wasm) throw new Error("Cargo did not emit the HTTP deadline WASM probe");
  run("wasm-bindgen", [wasm, "--target", "web", "--out-dir", temporary]);

  server = http.createServer(async (req, res) => {
    if (req.url === "/stall-headers") return;
    if (req.url === "/stall-body") {
      res.writeHead(200, { "content-type": "application/grpc" });
      res.write("partial");
      return;
    }
    if (req.url === "/ok") { res.end("ok"); return; }
    if (req.url === "/fail") { res.destroy(); return; }
    if (req.url === "/") {
      res.setHeader("content-type", "text/html"); res.end("<body>HTTP deadline probe</body>"); return;
    }
    const files = {
      "/http_deadline_probe.js": ["http_deadline_probe.js", "text/javascript"],
      "/http_deadline_probe_bg.wasm": ["http_deadline_probe_bg.wasm", "application/wasm"],
    };
    const file = files[req.url];
    if (!file) { res.writeHead(404); res.end(); return; }
    try {
      res.setHeader("content-type", file[1]); res.end(await fs.readFile(path.join(temporary, file[0])));
    } catch { res.writeHead(500); res.end(); }
  });
  server.on("connection", socket => {
    sockets.add(socket); socket.on("close", () => sockets.delete(socket));
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject); server.listen(0, "127.0.0.1", resolve);
  });
  const engine = process.env.CLIENT_RS_BROWSER || "chromium";
  const browserType = { chromium, firefox }[engine];
  if (!browserType) throw new Error(`Unsupported browser: ${engine}`);
  browser = await browserType.launch({
    headless: true,
    ...(process.env.CLIENT_RS_BROWSER_EXECUTABLE ? { executablePath: process.env.CLIENT_RS_BROWSER_EXECUTABLE } : {}),
    ...(engine === "chromium" ? { args: ["--no-sandbox"] } : {}),
  });
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  const results = await Promise.race([
    page.evaluate(async () => {
      const { default: init, probe, cancel } = await import("/http_deadline_probe.js");
      await init();
      const active = new Set();
      const originalSet = window.setTimeout.bind(window), originalClear = window.clearTimeout.bind(window);
      window.setTimeout = (fn, ms, ...args) => {
        const id = originalSet(() => { active.delete(id); fn(...args); }, ms);
        if (ms === 150) active.add(id);
        return id;
      };
      window.clearTimeout = id => { active.delete(id); originalClear(id); };
      const results = [];
      for (const route of ["/stall-headers", "/stall-body", "/ok", "/fail"]) {
        const start = performance.now();
        const failed = await probe(location.origin + route);
        results.push({ route, failed, elapsedMs: Math.round(performance.now() - start), activeTimers: active.size });
      }
      results.push({ route: "/cancel", failed: cancel(location.origin + "/stall-headers"), elapsedMs: 0, activeTimers: active.size });
      return results;
    }),
    new Promise((_, reject) => { const timer = setTimeout(() => reject(new Error("Deadline probe exceeded 10 seconds")), 10_000); timer.unref(); }),
  ]);
  console.log(JSON.stringify({ browser: engine, results }, null, 2));
  for (const result of results) {
    if (result.activeTimers !== 0 || result.failed !== (result.route !== "/ok") || result.elapsedMs > 2000) {
      throw new Error(`HTTP deadline assertion failed: ${JSON.stringify(result)}`);
    }
    if (result.route.startsWith("/stall-") && result.elapsedMs < 100) {
      throw new Error(`Stalled request failed before its deadline: ${JSON.stringify(result)}`);
    }
  }
} finally {
  await browser?.close();
  for (const socket of sockets) socket.destroy();
  if (server?.listening) await new Promise(resolve => server.close(resolve));
  await fs.rm(temporary, { recursive: true, force: true });
}
