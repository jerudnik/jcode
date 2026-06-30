#!/usr/bin/env node
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtemp, readFile, rm, writeFile, mkdir } from "node:fs/promises";
import { existsSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const WEB_ROOT = path.join(ROOT, "web", "jcode-mobile");
const OUTPUT_DIR = path.join(ROOT, ".tmp", "web-mobile-rendered");
const STATE_KEY = "jcode.mobileWeb.surfaceState.v1";
const DEFAULT_TIMEOUT_MS = 20000;

const VIEWPORTS = [
  { name: "key2", width: 390, height: 844, deviceScaleFactor: 2.7, isMobile: true },
  { name: "y700", width: 800, height: 1280, deviceScaleFactor: 2, isMobile: true },
  { name: "laptop", width: 1440, height: 1000, deviceScaleFactor: 1, isMobile: false },
];

const MIME_TYPES = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".mjs", "text/javascript; charset=utf-8"],
  [".css", "text/css; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".png", "image/png"],
  [".svg", "image/svg+xml"],
]);

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function chromeCandidates() {
  return [
    process.env.CHROME_PATH,
    process.env.GOOGLE_CHROME_SHIM,
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/usr/bin/google-chrome",
    "/usr/bin/google-chrome-stable",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
  ].filter(Boolean);
}

function findChrome() {
  for (const candidate of chromeCandidates()) {
    if (existsSync(candidate)) return candidate;
  }
  throw new Error("Chrome/Chromium not found. Set CHROME_PATH to a Chrome executable.");
}

function statusCodeForError(error) {
  if (error && error.code === "ENOENT") return 404;
  if (error && error.code === "EACCES") return 403;
  return 500;
}

async function createStaticServer(root) {
  const safeRoot = path.resolve(root);
  const server = createServer(async (request, response) => {
    try {
      const rawPath = new URL(request.url || "/", "http://localhost").pathname;
      if (rawPath === "/favicon.ico") {
        response.writeHead(204, { "cache-control": "no-store" });
        response.end();
        return;
      }
      const normalized = path.normalize(decodeURIComponent(rawPath)).replace(/^\.\.(?:[/\\]|$)/, "");
      const relative = normalized === path.sep ? "index.html" : normalized.replace(/^[/\\]/, "");
      const filePath = path.resolve(safeRoot, relative || "index.html");
      if (filePath !== safeRoot && !filePath.startsWith(`${safeRoot}${path.sep}`)) {
        response.writeHead(403).end("forbidden");
        return;
      }
      const body = await readFile(filePath);
      response.writeHead(200, {
        "content-type": MIME_TYPES.get(path.extname(filePath)) || "application/octet-stream",
        "cache-control": "no-store",
      });
      response.end(body);
    } catch (error) {
      response.writeHead(statusCodeForError(error), { "content-type": "text/plain; charset=utf-8" });
      response.end(error && error.code === "ENOENT" ? "not found" : String(error));
    }
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  return { server, origin: `http://127.0.0.1:${address.port}` };
}

async function waitForProcessExit(child, timeoutMs) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  await Promise.race([
    new Promise((resolve) => child.once("exit", resolve)),
    sleep(timeoutMs),
  ]);
}

async function waitForDevToolsPort(userDataDir, timeoutMs = DEFAULT_TIMEOUT_MS) {
  const activePortPath = path.join(userDataDir, "DevToolsActivePort");
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const text = await readFile(activePortPath, "utf8");
      const [port, browserPath] = text.trim().split(/\r?\n/);
      if (port && browserPath) return `ws://127.0.0.1:${port}${browserPath}`;
    } catch {}
    await sleep(50);
  }
  throw new Error("Timed out waiting for Chrome DevToolsActivePort");
}

async function launchChrome() {
  const chrome = findChrome();
  const tempRoot = await mkdtemp(path.join(tmpdir(), "jcode-render-smoke-"));
  const userDataDir = path.join(tempRoot, "profile");
  const stderr = [];
  const child = spawn(chrome, [
    "--headless=new",
    "--disable-gpu",
    "--disable-background-networking",
    "--disable-default-apps",
    "--disable-dev-shm-usage",
    "--disable-extensions",
    "--disable-popup-blocking",
    "--no-default-browser-check",
    "--no-first-run",
    "--remote-debugging-port=0",
    `--user-data-dir=${userDataDir}`,
    "about:blank",
  ], { stdio: ["ignore", "ignore", "pipe"] });
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => stderr.push(chunk));
  const wsUrl = await waitForDevToolsPort(userDataDir);
  return {
    wsUrl,
    child,
    tempRoot,
    async close() {
      if (child.exitCode === null && child.signalCode === null) child.kill("SIGTERM");
      await waitForProcessExit(child, 1000);
      if (child.exitCode === null && child.signalCode === null) {
        child.kill("SIGKILL");
        await waitForProcessExit(child, 1000);
      }
      await rm(tempRoot, { recursive: true, force: true });
    },
    stderr() { return stderr.join(""); },
  };
}

class CdpConnection {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.ws = null;
    this.nextId = 1;
    this.pending = new Map();
    this.events = [];
  }

  async connect() {
    if (typeof WebSocket !== "function") {
      throw new Error("Node global WebSocket is unavailable; use Node 22+ or install a CDP client.");
    }
    this.ws = new WebSocket(this.wsUrl);
    this.ws.addEventListener("message", (event) => this.handleMessage(event.data));
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("Timed out connecting to Chrome CDP")), DEFAULT_TIMEOUT_MS);
      this.ws.addEventListener("open", () => { clearTimeout(timer); resolve(); }, { once: true });
      this.ws.addEventListener("error", () => { clearTimeout(timer); reject(new Error("Chrome CDP websocket failed")); }, { once: true });
    });
  }

  handleMessage(raw) {
    const message = JSON.parse(String(raw));
    if (message.id && this.pending.has(message.id)) {
      const { resolve, reject } = this.pending.get(message.id);
      this.pending.delete(message.id);
      if (message.error) reject(new Error(`${message.error.message || "CDP error"}: ${message.error.data || ""}`));
      else resolve(message.result || {});
      return;
    }
    this.events.push(message);
  }

  send(method, params = {}, sessionId = null) {
    const id = this.nextId;
    this.nextId += 1;
    const message = { id, method, params };
    if (sessionId) message.sessionId = sessionId;
    this.ws.send(JSON.stringify(message));
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      setTimeout(() => {
        if (!this.pending.has(id)) return;
        this.pending.delete(id);
        reject(new Error(`Timed out waiting for CDP ${method}`));
      }, DEFAULT_TIMEOUT_MS);
    });
  }

  async waitForEvent(method, sessionId = null, timeoutMs = DEFAULT_TIMEOUT_MS) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      const index = this.events.findIndex((event) => event.method === method && (!sessionId || event.sessionId === sessionId));
      if (index >= 0) return this.events.splice(index, 1)[0];
      await sleep(25);
    }
    throw new Error(`Timed out waiting for CDP event ${method}`);
  }

  async close() {
    if (this.ws) this.ws.close();
  }
}

function expressionSource(source) {
  return `(() => { ${source} })()`;
}

async function evaluate(cdp, sessionId, expression, awaitPromise = false) {
  const result = await cdp.send("Runtime.evaluate", {
    expression,
    awaitPromise,
    returnByValue: true,
  }, sessionId);
  if (result.exceptionDetails) {
    throw new Error(result.exceptionDetails.text || "Runtime.evaluate exception");
  }
  return result.result ? result.result.value : undefined;
}

async function waitForExpression(cdp, sessionId, expression, description, timeoutMs = DEFAULT_TIMEOUT_MS) {
  const deadline = Date.now() + timeoutMs;
  let lastError = "";
  while (Date.now() < deadline) {
    try {
      const value = await evaluate(cdp, sessionId, expression);
      if (value) return value;
    } catch (error) {
      lastError = error.message || String(error);
    }
    await sleep(100);
  }
  throw new Error(`Timed out waiting for ${description}${lastError ? `; last error: ${lastError}` : ""}`);
}

function preloadScript() {
  return `
    window.__jcodeSmokeErrors = [];
    window.addEventListener("error", function(event) {
      window.__jcodeSmokeErrors.push(event.message || String(event.error || "error"));
    });
    window.addEventListener("unhandledrejection", function(event) {
      window.__jcodeSmokeErrors.push(String(event.reason || "unhandled rejection"));
    });
  `;
}

async function prepareTarget(cdp, viewport) {
  const target = await cdp.send("Target.createTarget", { url: "about:blank" });
  const attached = await cdp.send("Target.attachToTarget", { targetId: target.targetId, flatten: true });
  const sessionId = attached.sessionId;
  await cdp.send("Page.enable", {}, sessionId);
  await cdp.send("Runtime.enable", {}, sessionId);
  await cdp.send("Log.enable", {}, sessionId);
  await cdp.send("Emulation.setDeviceMetricsOverride", {
    width: viewport.width,
    height: viewport.height,
    deviceScaleFactor: viewport.deviceScaleFactor,
    mobile: viewport.isMobile,
    screenOrientation: {
      type: viewport.height >= viewport.width ? "portraitPrimary" : "landscapePrimary",
      angle: viewport.height >= viewport.width ? 0 : 90,
    },
  }, sessionId);
  await cdp.send("Page.addScriptToEvaluateOnNewDocument", { source: preloadScript() }, sessionId);
  return { targetId: target.targetId, sessionId };
}

function consoleFailures(cdp, sessionId) {
  return cdp.events
    .filter((event) => event.sessionId === sessionId)
    .filter((event) => event.method === "Runtime.exceptionThrown" || event.method === "Log.entryAdded")
    .map((event) => {
      if (event.method === "Runtime.exceptionThrown") {
        return event.params.exceptionDetails.text || event.params.exceptionDetails.exception.description || "Runtime exception";
      }
      const entry = event.params.entry || {};
      if (["error", "warning"].indexOf(entry.level) >= 0) return `${entry.level}: ${entry.text}`;
      return "";
    })
    .filter(Boolean)
    .filter((text) => !/favicon\.ico|Autofill/.test(text));
}

async function runViewport(cdp, origin, viewport) {
  const { targetId, sessionId } = await prepareTarget(cdp, viewport);
  const commandText = `render smoke ${viewport.name} ${Date.now()}`;
  const url = `${origin}/index.html?render-smoke=${encodeURIComponent(viewport.name)}`;
  await cdp.send("Page.navigate", { url }, sessionId);
  await waitForExpression(cdp, sessionId, "document.readyState === 'complete'", `${viewport.name} document complete`);
  await waitForExpression(cdp, sessionId, "Boolean(document.querySelector('.shell') && document.getElementById('composer-input'))", `${viewport.name} app shell`);

  await evaluate(cdp, sessionId, expressionSource(`
    localStorage.removeItem(${JSON.stringify(STATE_KEY)});
    const input = document.getElementById("composer-input");
    input.value = ${JSON.stringify(commandText)};
    input.dispatchEvent(new Event("input", { bubbles: true }));
    const form = input.closest("form");
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    return true;
  `));

  await waitForExpression(cdp, sessionId, `document.body.innerText.indexOf(${JSON.stringify(commandText)}) >= 0`, `${viewport.name} pending command visible`);
  const beforeReload = await collectViewportState(cdp, sessionId, commandText);
  await cdp.send("Page.reload", { ignoreCache: true }, sessionId);
  await waitForExpression(cdp, sessionId, "document.readyState === 'complete'", `${viewport.name} reload complete`);
  await waitForExpression(cdp, sessionId, "Boolean(document.querySelector('.shell') && document.getElementById('composer-input'))", `${viewport.name} shell after reload`);
  await waitForExpression(cdp, sessionId, `document.body.innerText.indexOf(${JSON.stringify(commandText)}) >= 0`, `${viewport.name} pending command persisted after reload`);
  const afterReload = await collectViewportState(cdp, sessionId, commandText);
  const screenshot = await cdp.send("Page.captureScreenshot", { format: "png", fromSurface: true, captureBeyondViewport: false }, sessionId);
  const screenshotPath = path.join(OUTPUT_DIR, `${viewport.name}.png`);
  await writeFile(screenshotPath, Buffer.from(screenshot.data, "base64"));
  await cdp.send("Target.closeTarget", { targetId });

  const errors = [...beforeReload.smokeErrors, ...afterReload.smokeErrors, ...consoleFailures(cdp, sessionId)];
  const failures = [];
  if (errors.length) failures.push(`runtime errors: ${errors.join(" | ")}`);
  if (beforeReload.hasOverflow) failures.push(`horizontal overflow before reload: ${beforeReload.scrollWidth} > ${beforeReload.innerWidth}`);
  if (afterReload.hasOverflow) failures.push(`horizontal overflow after reload: ${afterReload.scrollWidth} > ${afterReload.innerWidth}`);
  if (!beforeReload.pendingVisible || !afterReload.pendingVisible) failures.push("pending command not visible across queue/reload");
  if (!beforeReload.persisted || !afterReload.persisted) failures.push("pending command not persisted in localStorage");

  return {
    viewport,
    commandText,
    screenshotPath: path.relative(ROOT, screenshotPath),
    beforeReload,
    afterReload,
    errors,
    failures,
  };
}

async function collectViewportState(cdp, sessionId, commandText) {
  return evaluate(cdp, sessionId, expressionSource(`
    const root = document.documentElement;
    const body = document.body;
    const maxScrollWidth = Math.max(root.scrollWidth, body ? body.scrollWidth : 0);
    const pendingVisible = document.body.innerText.indexOf(${JSON.stringify(commandText)}) >= 0;
    let persisted = false;
    try {
      const parsed = JSON.parse(localStorage.getItem(${JSON.stringify(STATE_KEY)}) || "{}");
      persisted = Array.isArray(parsed.pendingCommands) && parsed.pendingCommands.some(function(command) {
        return command && command.payload && String(command.payload.content).indexOf(${JSON.stringify(commandText)}) >= 0;
      });
    } catch {}
    return {
      innerWidth: window.innerWidth,
      innerHeight: window.innerHeight,
      scrollWidth: maxScrollWidth,
      hasOverflow: maxScrollWidth > window.innerWidth + 1,
      pendingVisible,
      persisted,
      appRendered: Boolean(document.querySelector(".shell")),
      smokeErrors: Array.isArray(window.__jcodeSmokeErrors) ? window.__jcodeSmokeErrors.slice() : [],
    };
  `));
}

async function main() {
  await mkdir(OUTPUT_DIR, { recursive: true });
  const { server, origin } = await createStaticServer(WEB_ROOT);
  const chrome = await launchChrome();
  const cdp = new CdpConnection(chrome.wsUrl);
  const report = { origin, generatedAt: new Date().toISOString(), viewports: [] };
  try {
    await cdp.connect();
    for (const viewport of VIEWPORTS) {
      process.stdout.write(`render smoke ${viewport.name} ${viewport.width}x${viewport.height}... `);
      const result = await runViewport(cdp, origin, viewport);
      report.viewports.push(result);
      if (result.failures.length) {
        console.log("failed");
      } else {
        console.log("ok");
      }
    }
  } finally {
    await writeFile(path.join(OUTPUT_DIR, "report.json"), JSON.stringify(report, null, 2));
    await cdp.close().catch(() => {});
    await chrome.close().catch(() => {});
    await new Promise((resolve) => server.close(resolve));
  }

  const failures = report.viewports.flatMap((item) => item.failures.map((failure) => `${item.viewport.name}: ${failure}`));
  console.log(`rendered smoke report: ${path.relative(ROOT, path.join(OUTPUT_DIR, "report.json"))}`);
  if (failures.length) {
    failures.forEach((failure) => console.error(failure));
    process.exitCode = 1;
  }
}

main().catch((error) => {
  console.error(error && error.stack ? error.stack : String(error));
  process.exit(1);
});
