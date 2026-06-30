const MAX_COMMAND_LOG = 120;
const COMMAND_ID_PREFIX = "cmd";

export const COMMAND_LOG_VERSION = 1;

export const COMMAND_VERBS = [
  "message.send",
  "turn.cancel",
  "history.sync",
  "session.attach",
  "session.switch",
  "model.set",
  "intent.capture",
  "intent.route",
  "agent.spawn",
  "agent.assign",
  "artifact.open",
  "annotation.create",
  "card.create",
  "card.move",
  "doc.create",
  "surface.handoff",
  "summary.request",
  "agent.meta",
];

const ALIASES = {
  send: "message.send",
  message: "message.send",
  cancel: "turn.cancel",
  sync: "history.sync",
  history: "history.sync",
  attach: "session.attach",
  switch: "session.switch",
  model: "model.set",
  intent: "intent.capture",
  route: "intent.route",
  spawn: "agent.spawn",
  assign: "agent.assign",
  artifact: "artifact.open",
  annotate: "annotation.create",
  annotation: "annotation.create",
  card: "card.create",
  move: "card.move",
  doc: "doc.create",
  handoff: "surface.handoff",
  summary: "summary.request",
  meta: "agent.meta",
};

function coerceString(value) {
  return typeof value === "string" ? value : "";
}

function nowIso(now) {
  if (now instanceof Date) return now.toISOString();
  if (typeof now === "number") return new Date(now).toISOString();
  return new Date().toISOString();
}

export function makeCommandId(now, randomFn) {
  const timestamp = typeof now === "number" ? now : Date.now();
  const random = typeof randomFn === "function" ? randomFn() : Math.random();
  const suffix = Math.floor(random * 0xffffff).toString(16).padStart(6, "0");
  return `${COMMAND_ID_PREFIX}_${timestamp.toString(36)}_${suffix}`;
}

function knownVerb(verb) {
  return COMMAND_VERBS.indexOf(verb) >= 0;
}

function normalizeVerb(raw) {
  const cleaned = coerceString(raw).trim().replace(/^\//, "");
  if (!cleaned) return "";
  if (ALIASES[cleaned]) return ALIASES[cleaned];
  return cleaned;
}

function parseJsonPayload(rest) {
  const text = rest.trim();
  if (!text || text.charAt(0) !== "{") return null;
  try {
    const parsed = JSON.parse(text);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function splitWords(rest) {
  return rest.trim().split(/\s+/).filter(Boolean);
}

function titleAndBody(rest) {
  const parts = rest.split("|").map((part) => part.trim());
  return {
    title: parts[0] || "Untitled",
    body: parts.slice(1).join(" | "),
  };
}

export function payloadForVerb(verb, rest, context) {
  const json = parseJsonPayload(rest);
  if (json) return json;
  const words = splitWords(rest);
  const activeSession = context && context.sessionId ? context.sessionId : "";
  if (verb === "message.send") return { session_id: activeSession, content: rest.trim() };
  if (verb === "turn.cancel") return { session_id: words[0] || activeSession };
  if (verb === "history.sync") return { session_id: words[0] || activeSession };
  if (verb === "session.attach" || verb === "session.switch") return { session_id: words[0] || activeSession };
  if (verb === "model.set") return { model: rest.trim() };
  if (verb === "intent.capture") return { body: rest.trim(), urgency: "normal" };
  if (verb === "intent.route") return { intent_id: words[0] || "", body: rest.trim(), target: words[1] || "" };
  if (verb === "agent.spawn") return { prompt: rest.trim(), role: "agent" };
  if (verb === "agent.assign") return { task_id: words[0] || "", role: words[1] || "agent" };
  if (verb === "artifact.open") return { path: rest.trim(), artifact_id: rest.trim() };
  if (verb === "annotation.create") return { target: { kind: "workspace" }, body: rest.trim(), kind: "note" };
  if (verb === "card.create") return titleAndBody(rest);
  if (verb === "card.move") return { card_id: words[0] || "", status: words[1] || "todo", ordinal: words[2] || "" };
  if (verb === "doc.create") return titleAndBody(rest);
  if (verb === "surface.handoff") return { target_surface: words[0] || "tui", context: rest.trim() };
  if (verb === "summary.request") return { scope: rest.trim() || "workspace" };
  if (verb === "agent.meta") return { instruction: rest.trim(), agent_id: words[0] || "" };
  return {};
}

export function parseCommandInput(input, context) {
  const text = coerceString(input).trim();
  if (!text) return { ok: false, error: "Command is empty", raw: input };
  if (text.charAt(0) !== "/") {
    return {
      ok: true,
      verb: "message.send",
      payload: payloadForVerb("message.send", text, context),
      raw: input,
    };
  }
  const match = text.match(/^\/(\S+)(?:\s+([\s\S]*))?$/);
  if (!match) return { ok: false, error: "Could not parse command", raw: input };
  let verb = normalizeVerb(match[1]);
  let rest = match[2] || "";
  const subcommand = rest.trim().match(/^(\S+)(?:\s+([\s\S]*))?$/);
  if (match[1] === "card" && subcommand && subcommand[1] === "move") {
    verb = "card.move";
    rest = subcommand[2] || "";
  } else if (match[1] === "card" && subcommand && subcommand[1] === "create") {
    verb = "card.create";
    rest = subcommand[2] || "";
  } else if (match[1] === "doc" && subcommand && subcommand[1] === "create") {
    verb = "doc.create";
    rest = subcommand[2] || "";
  } else if (match[1] === "artifact" && subcommand && subcommand[1] === "open") {
    verb = "artifact.open";
    rest = subcommand[2] || "";
  } else if (match[1] === "summary" && subcommand && subcommand[1] === "request") {
    verb = "summary.request";
    rest = subcommand[2] || "";
  } else if (match[1] === "intent" && subcommand && subcommand[1] === "route") {
    verb = "intent.route";
    rest = subcommand[2] || "";
  }
  if (!knownVerb(verb)) {
    return { ok: false, error: `Unknown command verb: ${verb}`, raw: input, verb };
  }
  const payload = payloadForVerb(verb, rest, context);
  if (verb === "message.send" && !payload.content) return { ok: false, error: "message.send needs content", raw: input, verb };
  if (verb === "model.set" && !payload.model) return { ok: false, error: "model.set needs a model", raw: input, verb };
  if (verb === "card.move" && !payload.card_id) return { ok: false, error: "card.move needs a card id", raw: input, verb };
  return { ok: true, verb, payload, raw: input };
}

export function createCommandEnvelope(parsed, options) {
  const now = options && options.now ? options.now : new Date();
  const randomFn = options && options.randomFn ? options.randomFn : Math.random;
  const createdAt = nowIso(now);
  return {
    id: options && options.id ? options.id : makeCommandId(now instanceof Date ? now.getTime() : Date.now(), randomFn),
    verb: parsed.verb,
    surface_id: options && options.surfaceId ? options.surfaceId : "surface_browser",
    target_session_id: options && options.sessionId ? options.sessionId : "",
    payload: parsed.payload || {},
    status: options && options.status ? options.status : "pending",
    request_id: null,
    created_at: createdAt,
    updated_at: createdAt,
    error: "",
    raw: coerceString(parsed.raw),
  };
}

export function normalizeCommandLog(value) {
  const source = Array.isArray(value) ? value : [];
  const normalized = [];
  for (let index = 0; index < source.length && normalized.length < MAX_COMMAND_LOG; index += 1) {
    const item = source[index];
    if (!item || typeof item !== "object") continue;
    const verb = normalizeVerb(item.verb);
    if (!knownVerb(verb)) continue;
    const status = ["pending", "sending", "acked", "failed", "replayed"].indexOf(item.status) >= 0 ? item.status : "pending";
    normalized.push({
      id: coerceString(item.id) || makeCommandId(Date.now(), Math.random),
      verb,
      surface_id: coerceString(item.surface_id) || "surface_browser",
      target_session_id: coerceString(item.target_session_id),
      payload: item.payload && typeof item.payload === "object" ? item.payload : {},
      status,
      request_id: Number.isInteger(item.request_id) ? item.request_id : null,
      created_at: coerceString(item.created_at) || nowIso(),
      updated_at: coerceString(item.updated_at) || nowIso(),
      error: coerceString(item.error),
      raw: coerceString(item.raw),
    });
  }
  return normalized;
}

export function restoreCommandLog(raw) {
  if (!raw) return [];
  let parsed = raw;
  if (typeof raw === "string") {
    try {
      parsed = JSON.parse(raw);
    } catch {
      return [];
    }
  }
  if (parsed && typeof parsed === "object" && Array.isArray(parsed.commands)) {
    return normalizeCommandLog(parsed.commands);
  }
  return normalizeCommandLog(parsed);
}

export function serializeCommandLog(commands) {
  return JSON.stringify({ version: COMMAND_LOG_VERSION, commands: normalizeCommandLog(commands) });
}

export function appendCommandLog(commands, envelope) {
  const next = normalizeCommandLog(commands);
  next.push(envelope);
  while (next.length > MAX_COMMAND_LOG) next.shift();
  return normalizeCommandLog(next);
}

export function markCommandLogStatus(commands, commandId, status, error, requestId, now) {
  return normalizeCommandLog(commands).map((command) => {
    if (command.id !== commandId) return command;
    return Object.assign({}, command, {
      status,
      request_id: Number.isInteger(requestId) ? requestId : command.request_id,
      error: coerceString(error),
      updated_at: nowIso(now),
    });
  });
}

export function commandLogSummary(commands) {
  const items = normalizeCommandLog(commands);
  if (!items.length) return "no commands";
  const pending = items.filter((command) => command.status === "pending" || command.status === "sending").length;
  const failed = items.filter((command) => command.status === "failed").length;
  if (failed) return `${failed} failed / ${items.length} logged`;
  if (pending) return `${pending} pending / ${items.length} logged`;
  return `${items.length} logged`;
}
