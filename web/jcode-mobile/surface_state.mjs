const MAX_PENDING_COMMANDS = 30;
const RECONNECT_BASE_MS = 750;
const RECONNECT_MAX_MS = 15000;
const RECONNECT_JITTER_RATIO = 0.25;
const KNOWN_INBOUND_EVENTS = [
  "ack",
  "history",
  "session",
  "session_renamed",
  "state",
  "available_models_updated",
  "model_changed",
  "tokens",
  "status_detail",
  "notification",
  "reasoning_delta",
  "reasoning_done",
  "text_delta",
  "text_replace",
  "tool_start",
  "tool_input",
  "tool_exec",
  "tool_done",
  "message_end",
  "done",
  "interrupted",
  "error",
];

export const SURFACE_STATE_VERSION = 1;

export const PHASE_LABELS = {
  offline: "offline",
  reconnecting: "reconnecting",
  resyncing: "resyncing",
  live: "live",
  idle: "idle session",
  auth_failure: "auth failure",
  error: "error",
};

function coerceString(value) {
  return typeof value === "string" ? value : "";
}

function coerceBoolean(value) {
  return value === true;
}

function nowIso(now) {
  if (now instanceof Date) return now.toISOString();
  if (typeof now === "number") return new Date(now).toISOString();
  return new Date().toISOString();
}

function commandContent(command) {
  if (!command || !command.payload) return "";
  return coerceString(command.payload.content);
}

export function normalizePendingCommands(value) {
  if (!Array.isArray(value)) return [];
  const normalized = [];
  for (let index = 0; index < value.length && normalized.length < MAX_PENDING_COMMANDS; index += 1) {
    const item = value[index];
    if (!item || typeof item !== "object") continue;
    const verb = item.verb === "message.send" ? "message.send" : "";
    const content = commandContent(item).trim();
    if (!verb || !content) continue;
    const status = ["queued", "sending", "unknown", "failed"].indexOf(item.status) >= 0 ? item.status : "queued";
    normalized.push({
      id: coerceString(item.id) || makeCommandId(Date.now(), Math.random),
      verb,
      request_id: Number.isInteger(item.request_id) ? item.request_id : null,
      session_id: coerceString(item.session_id),
      payload: { content },
      status,
      attempts: Number.isInteger(item.attempts) && item.attempts >= 0 ? item.attempts : 0,
      created_at: coerceString(item.created_at) || nowIso(),
      updated_at: coerceString(item.updated_at) || nowIso(),
      last_error: coerceString(item.last_error),
    });
  }
  return normalized;
}

export function defaultSurfaceState() {
  return {
    version: SURFACE_STATE_VERSION,
    activeId: "",
    draft: "",
    sessionId: "",
    focusMode: false,
    sessionFilter: "",
    modelFilter: "",
    lastSyncAt: "",
    pendingCommands: [],
  };
}

export function restoreSurfaceState(raw) {
  const base = defaultSurfaceState();
  if (!raw) return base;
  let parsed = raw;
  if (typeof raw === "string") {
    try {
      parsed = JSON.parse(raw);
    } catch {
      return base;
    }
  }
  if (!parsed || typeof parsed !== "object") return base;
  return {
    version: SURFACE_STATE_VERSION,
    activeId: coerceString(parsed.activeId),
    draft: coerceString(parsed.draft),
    sessionId: coerceString(parsed.sessionId),
    focusMode: coerceBoolean(parsed.focusMode),
    sessionFilter: coerceString(parsed.sessionFilter),
    modelFilter: coerceString(parsed.modelFilter),
    lastSyncAt: coerceString(parsed.lastSyncAt),
    pendingCommands: normalizePendingCommands(parsed.pendingCommands),
  };
}

export function surfaceStateSnapshot(input) {
  return {
    version: SURFACE_STATE_VERSION,
    activeId: coerceString(input.activeId),
    draft: coerceString(input.draft),
    sessionId: coerceString(input.sessionId),
    focusMode: coerceBoolean(input.focusMode),
    sessionFilter: coerceString(input.sessionFilter),
    modelFilter: coerceString(input.modelFilter),
    lastSyncAt: coerceString(input.lastSyncAt),
    pendingCommands: normalizePendingCommands(input.pendingCommands),
  };
}

export function serializeSurfaceState(input) {
  return JSON.stringify(surfaceStateSnapshot(input));
}

export function makeCommandId(now, randomFn) {
  const timestamp = typeof now === "number" ? now : Date.now();
  const random = typeof randomFn === "function" ? randomFn() : Math.random();
  const suffix = Math.floor(random * 0xffffff).toString(16).padStart(6, "0");
  return `cmd_${timestamp.toString(36)}_${suffix}`;
}

export function createPendingMessageCommand(options) {
  const now = options && options.now ? options.now : new Date();
  const randomFn = options && options.randomFn ? options.randomFn : Math.random;
  const createdAt = nowIso(now);
  return {
    id: options && options.id ? options.id : makeCommandId(now instanceof Date ? now.getTime() : Date.now(), randomFn),
    verb: "message.send",
    request_id: null,
    session_id: options && options.sessionId ? options.sessionId : "",
    payload: { content: options && options.content ? String(options.content).trim() : "" },
    status: "queued",
    attempts: 0,
    created_at: createdAt,
    updated_at: createdAt,
    last_error: "",
  };
}

export function appendPendingCommand(commands, command) {
  const next = normalizePendingCommands(commands);
  if (!command || !command.payload || !String(command.payload.content || "").trim()) return next;
  next.push(command);
  while (next.length > MAX_PENDING_COMMANDS) next.shift();
  return normalizePendingCommands(next);
}

export function commandRequestPayload(command, requestId) {
  if (!command || command.verb !== "message.send") return null;
  return {
    type: "message",
    id: requestId,
    content: commandContent(command),
  };
}

export function markCommandSending(commands, commandId, requestId, now) {
  return normalizePendingCommands(commands).map((command) => {
    if (command.id !== commandId) return command;
    return Object.assign({}, command, {
      request_id: requestId,
      status: "sending",
      attempts: command.attempts + 1,
      updated_at: nowIso(now),
      last_error: "",
    });
  });
}

export function markCommandFailed(commands, commandId, error, now) {
  return normalizePendingCommands(commands).map((command) => {
    if (command.id !== commandId) return command;
    return Object.assign({}, command, {
      request_id: null,
      status: "failed",
      updated_at: nowIso(now),
      last_error: coerceString(error) || "send failed",
    });
  });
}

export function markCommandQueued(commands, commandId, now) {
  return normalizePendingCommands(commands).map((command) => {
    if (command.id !== commandId) return command;
    return Object.assign({}, command, {
      request_id: null,
      status: "queued",
      updated_at: nowIso(now),
      last_error: "",
    });
  });
}

export function markInflightCommandsUnknown(commands, now, reason) {
  return normalizePendingCommands(commands).map((command) => {
    if (command.status !== "sending") return command;
    return Object.assign({}, command, {
      request_id: null,
      status: "unknown",
      updated_at: nowIso(now),
      last_error: coerceString(reason) || "connection closed before ack",
    });
  });
}

export function applyCommandAck(commands, requestId) {
  const source = normalizePendingCommands(commands);
  let ackedCommand = null;
  const remaining = [];
  for (let index = 0; index < source.length; index += 1) {
    const command = source[index];
    if (command.status === "sending" && command.request_id === requestId) {
      ackedCommand = command;
    } else {
      remaining.push(command);
    }
  }
  return { commands: remaining, ackedCommand };
}

export function removePendingCommand(commands, commandId) {
  return normalizePendingCommands(commands).filter((command) => command.id !== commandId);
}

export function pendingCommandSummary(commands) {
  const counts = { queued: 0, sending: 0, unknown: 0, failed: 0 };
  const source = normalizePendingCommands(commands);
  for (let index = 0; index < source.length; index += 1) {
    const status = source[index].status;
    if (counts[status] !== undefined) counts[status] += 1;
  }
  const parts = [];
  if (counts.queued) parts.push(`${counts.queued} queued`);
  if (counts.sending) parts.push(`${counts.sending} sending`);
  if (counts.unknown) parts.push(`${counts.unknown} needs review`);
  if (counts.failed) parts.push(`${counts.failed} failed`);
  return parts.length ? parts.join(" · ") : "none pending";
}

export function computeReconnectDelay(attempt, randomFn) {
  const safeAttempt = Math.max(0, Math.min(12, Number(attempt) || 0));
  const exponential = Math.min(RECONNECT_MAX_MS, RECONNECT_BASE_MS * Math.pow(2, safeAttempt));
  const random = typeof randomFn === "function" ? randomFn() : Math.random();
  const boundedRandom = Math.max(0, Math.min(1, Number(random) || 0));
  const jitter = exponential * RECONNECT_JITTER_RATIO * boundedRandom;
  return Math.min(RECONNECT_MAX_MS, Math.round(exponential + jitter));
}

export function isAuthFailureClose(code, reason) {
  if (code === 1008 || code === 4401 || code === 4403) return true;
  const text = coerceString(reason).toLowerCase();
  return text.indexOf("auth") >= 0 || text.indexOf("token") >= 0 || text.indexOf("unauthorized") >= 0 || text.indexOf("forbidden") >= 0;
}

export function classifySocketClose(input) {
  const manual = input && input.manual === true;
  const online = !input || input.online !== false;
  const visible = !input || input.visible !== false;
  const code = input && Number.isInteger(input.code) ? input.code : 0;
  const reason = input && input.reason ? input.reason : "";
  if (manual) {
    return { phase: "offline", status: "Disconnected", reconnect: false };
  }
  if (!visible) {
    return { phase: "offline", status: "Backgrounded. Local state saved; will reconnect and resync on foreground.", reconnect: false };
  }
  if (!online) {
    return { phase: "offline", status: "Offline. Drafts and pending commands are saved locally.", reconnect: false };
  }
  if (isAuthFailureClose(code, reason)) {
    return { phase: "auth_failure", status: "Auth failed. Forget and pair again if this saved token is stale.", reconnect: false };
  }
  return { phase: "reconnecting", status: "Connection closed. Reconnecting with backoff.", reconnect: true };
}

export function buildResyncRequests(options) {
  const requests = [];
  let nextId = options && Number.isInteger(options.nextId) ? options.nextId : 1;
  const subscribe = { type: "subscribe", id: nextId };
  nextId += 1;
  if (options && options.sessionId) subscribe.target_session_id = options.sessionId;
  if (options && options.clientInstanceId) subscribe.client_instance_id = options.clientInstanceId;
  if (options && options.hasLocalHistory) subscribe.client_has_local_history = true;
  if (options && options.sessionId) subscribe.allow_session_takeover = true;
  requests.push(subscribe);
  requests.push({ type: "get_history", id: nextId });
  nextId += 1;
  return { requests, nextId };
}

export function phaseLabel(phase) {
  return PHASE_LABELS[phase] || String(phase || "offline");
}

export function unknownProtocolEventStatus(event) {
  const type = event && typeof event.type === "string" && event.type ? event.type : "unknown";
  if (KNOWN_INBOUND_EVENTS.indexOf(type) >= 0) return "";
  return `Ignored ${type} event`;
}
