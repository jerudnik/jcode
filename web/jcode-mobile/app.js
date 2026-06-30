import { html, reactive, watch } from "https://esm.sh/@arrow-js/core@1.0.6";
import {
  appendPendingCommand,
  applyCommandAck,
  buildResyncRequests,
  classifySocketClose,
  commandRequestPayload,
  computeReconnectDelay,
  createPendingMessageCommand,
  markCommandFailed,
  markCommandQueued,
  markCommandSending,
  markInflightCommandsUnknown,
  pendingCommandSummary,
  phaseLabel,
  removePendingCommand,
  restoreSurfaceState,
  serializeSurfaceState,
  unknownProtocolEventStatus,
} from "./surface_state.mjs";

const STORAGE_KEY = "jcode.mobileWeb.credentials.v1";
const DEVICE_ID_KEY = "jcode.mobileWeb.deviceId.v1";
const SURFACE_STATE_KEY = "jcode.mobileWeb.surfaceState.v1";

const persistedSurface = loadSurfaceState();

const state = reactive({
  host: "",
  port: "7643",
  code: "",
  deviceName: defaultDeviceName(),
  credentials: loadCredentials(),
  activeId: persistedSurface.activeId,
  phase: navigator.onLine === false ? "offline" : "offline",
  status: navigator.onLine === false ? "Offline. Drafts and pending commands are saved locally." : "Not connected",
  error: "",
  draft: persistedSurface.draft,
  sessionFilter: persistedSurface.sessionFilter,
  modelFilter: persistedSurface.modelFilter,
  focusMode: persistedSurface.focusMode,
  sessionId: persistedSurface.sessionId,
  sessionTitle: "",
  providerModel: "",
  providerName: "",
  availableModels: [],
  allSessions: [],
  tokens: "",
  transcript: [],
  nextId: 1,
  pairing: false,
  connecting: false,
  networkOnline: navigator.onLine !== false,
  pageVisible: document.visibilityState !== "hidden",
  reconnectAttempt: 0,
  reconnectDelayMs: 0,
  reconnectDueAt: 0,
  lastSyncAt: persistedSurface.lastSyncAt,
  pendingCommands: persistedSurface.pendingCommands,
});

let socket = null;
let currentAssistantId = null;
let currentToolId = null;
let reconnectTimer = null;
let connectionSerial = 0;

if (state.credentials.length > 0) {
  let selected = activeCredential();
  if (!selected) {
    selected = state.credentials[state.credentials.length - 1];
    state.activeId = credentialId(selected);
  }
  state.host = selected.host;
  state.port = String(selected.port);
}

if (state.pendingCommands.some((command) => command.status === "sending")) {
  state.pendingCommands = markInflightCommandsUnknown(state.pendingCommands, new Date(), "Recovered after reload before ack");
}

function defaultDeviceName() {
  const platform = navigator.userAgent.includes("Android") ? "Android" : "browser";
  return `jcode ${platform}`;
}

function deviceId() {
  let id = localStorage.getItem(DEVICE_ID_KEY);
  if (!id) {
    id = window.crypto && window.crypto.randomUUID ? window.crypto.randomUUID() : `web-${Date.now()}-${Math.random().toString(16).slice(2)}`;
    localStorage.setItem(DEVICE_ID_KEY, id);
  }
  return id;
}

function loadCredentials() {
  try {
    const parsed = JSON.parse(localStorage.getItem(STORAGE_KEY) || "[]");
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function saveCredentials(credentials) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(credentials));
}

function loadSurfaceState() {
  try {
    return restoreSurfaceState(localStorage.getItem(SURFACE_STATE_KEY));
  } catch {
    return restoreSurfaceState(null);
  }
}

function persistSurfaceState() {
  try {
    localStorage.setItem(SURFACE_STATE_KEY, serializeSurfaceState({
      activeId: state.activeId,
      draft: state.draft,
      sessionId: state.sessionId,
      focusMode: state.focusMode,
      sessionFilter: state.sessionFilter,
      modelFilter: state.modelFilter,
      lastSyncAt: state.lastSyncAt,
      pendingCommands: state.pendingCommands,
    }));
  } catch {
    state.status = "Local storage unavailable; drafts may not survive reload.";
  }
}

function setDraft(value) {
  state.draft = value;
  persistSurfaceState();
}

function setPhase(phase, message) {
  state.phase = phase;
  state.status = message || phaseLabel(phase);
}

function documentIsVisible() {
  return document.visibilityState !== "hidden";
}

function credentialId(credential) {
  return `${credential.host}:${credential.port}`;
}

function activeCredential() {
  return state.credentials.find((credential) => credentialId(credential) === state.activeId) || null;
}

function gatewayBase(credential = null) {
  const host = credential && credential.host ? credential.host : state.host.trim();
  const port = credential && credential.port ? credential.port : Number(state.port || 7643);
  return { host, port, http: `http://${host}:${port}`, ws: `ws://${host}:${port}` };
}

function canPair() {
  return state.host.trim().length > 0 && state.code.trim().length > 0 && Number.isInteger(Number(state.port));
}

function setError(message) {
  state.error = message || "";
  if (message) state.status = message;
}

async function pair() {
  if (!canPair() || state.pairing) return;
  state.pairing = true;
  setError("");
  state.status = "Pairing...";
  try {
    const base = gatewayBase();
    const response = await fetch(`${base.http}/pair`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        code: state.code.trim(),
        device_id: deviceId(),
        device_name: state.deviceName.trim() || defaultDeviceName(),
      }),
    });
    const body = await response.json().catch(() => ({}));
    if (!response.ok) throw new Error(body.error || `Pairing failed with HTTP ${response.status}`);
    if (!body.token) throw new Error("Pairing response did not include a token");

    const credential = {
      host: base.host,
      port: base.port,
      token: body.token,
      serverName: body.server_name || "jcode",
      serverVersion: body.server_version || "unknown",
      pairedAt: new Date().toISOString(),
    };
    const next = state.credentials.filter((existing) => credentialId(existing) !== credentialId(credential));
    next.push(credential);
    state.credentials = next;
    state.activeId = credentialId(credential);
    saveCredentials(next);
    persistSurfaceState();
    state.code = "";
    state.status = `Paired with ${credential.serverName}`;
    connect("paired");
  } catch (error) {
    setError(error.message || String(error));
  } finally {
    state.pairing = false;
  }
}

function canUseSocket() {
  return socket && socket.readyState === WebSocket.OPEN;
}

function clearReconnectTimer() {
  if (reconnectTimer) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
  state.reconnectDelayMs = 0;
  state.reconnectDueAt = 0;
}

function connect(reason = "manual") {
  const credential = activeCredential();
  if (!credential || state.connecting) return;
  if (state.networkOnline === false) {
    setPhase("offline", "Offline. Drafts and pending commands are saved locally.");
    return;
  }
  if (state.pageVisible === false) {
    setPhase("offline", "Backgrounded. Local state saved; will reconnect and resync on foreground.");
    return;
  }
  clearReconnectTimer();
  connectionSerial += 1;
  const serial = connectionSerial;
  if (socket) {
    state.pendingCommands = markInflightCommandsUnknown(state.pendingCommands, new Date(), "Socket replaced before ack");
    persistSurfaceState();
    try { socket.close(); } catch {}
    socket = null;
  }
  state.connecting = true;
  setPhase("reconnecting", `Connecting to ${credential.host}:${credential.port} (${reason})...`);
  setError("");
  const base = gatewayBase(credential);
  const url = `${base.ws}/ws?token=${encodeURIComponent(credential.token)}`;
  let opened = false;
  socket = new WebSocket(url);
  socket.addEventListener("open", () => {
    if (serial !== connectionSerial) return;
    opened = true;
    state.connecting = false;
    state.reconnectAttempt = 0;
    setPhase("resyncing", "Connected. Resubscribing and fetching history...");
    resyncAfterReconnect(reason);
  });
  socket.addEventListener("message", (event) => {
    if (serial !== connectionSerial) return;
    String(event.data)
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean)
      .forEach(handleLine);
  });
  socket.addEventListener("close", (event) => {
    if (serial !== connectionSerial) return;
    state.connecting = false;
    state.pendingCommands = markInflightCommandsUnknown(state.pendingCommands, new Date(), "Connection closed before ack");
    persistSurfaceState();
    socket = null;
    const close = classifySocketClose({
      manual: false,
      online: state.networkOnline,
      visible: state.pageVisible,
      opened,
      code: event.code,
      reason: event.reason,
    });
    setPhase(close.phase, close.status);
    if (close.reconnect) scheduleReconnect("socket close");
  });
  socket.addEventListener("error", () => {
    if (serial !== connectionSerial) return;
    state.connecting = false;
    state.phase = "error";
    setError("WebSocket connection failed. Check gateway, token, LAN/Tailscale reachability, and mixed-content browser rules.");
  });
}

function disconnect(updateStatus = true) {
  clearReconnectTimer();
  connectionSerial += 1;
  if (socket) {
    socket.close();
    socket = null;
  }
  state.connecting = false;
  state.pendingCommands = markInflightCommandsUnknown(state.pendingCommands, new Date(), "Disconnected before ack");
  persistSurfaceState();
  if (updateStatus) setPhase("offline", "Disconnected");
}

function nextRequestId() {
  const id = state.nextId;
  state.nextId += 1;
  return id;
}

function sendRaw(payload) {
  if (!socket || socket.readyState !== WebSocket.OPEN) {
    throw new Error("Not connected");
  }
  socket.send(JSON.stringify(payload));
}

function sendResyncRequests() {
  const resync = buildResyncRequests({
    nextId: state.nextId,
    sessionId: state.sessionId,
    clientInstanceId: deviceId(),
    hasLocalHistory: state.transcript.length > 0,
  });
  state.nextId = resync.nextId;
  resync.requests.forEach(sendRaw);
}

function resyncAfterReconnect(reason) {
  try {
    sendResyncRequests();
    setPhase("resyncing", `Resyncing history after ${reason}...`);
  } catch (error) {
    setError(error.message || String(error));
    scheduleReconnect("resync send failed");
  }
}

function scheduleReconnect(reason, immediate = false) {
  if (!activeCredential()) return;
  if (state.networkOnline === false) {
    setPhase("offline", "Offline. Drafts and pending commands are saved locally.");
    return;
  }
  if (state.pageVisible === false) {
    setPhase("offline", "Backgrounded. Local state saved; will reconnect and resync on foreground.");
    return;
  }
  clearReconnectTimer();
  const delay = immediate ? 0 : computeReconnectDelay(state.reconnectAttempt, Math.random);
  state.reconnectAttempt += 1;
  state.reconnectDelayMs = delay;
  state.reconnectDueAt = Date.now() + delay;
  setPhase("reconnecting", delay > 0 ? `Reconnecting in ${Math.ceil(delay / 1000)}s (${reason})...` : `Reconnecting (${reason})...`);
  reconnectTimer = setTimeout(() => connect(reason), delay);
}

function finishHistoryResync() {
  state.lastSyncAt = new Date().toISOString();
  persistSurfaceState();
  if (state.sessionId) {
    setPhase("live", `Live. History synced ${new Date(state.lastSyncAt).toLocaleTimeString()}.`);
  } else {
    setPhase("idle", "Connected. No active session yet.");
  }
  flushQueuedCommands();
}

function flushQueuedCommands() {
  if (!canUseSocket()) return;
  const queued = state.pendingCommands.filter((command) => command.status === "queued");
  queued.forEach((command) => sendPendingCommand(command.id, "auto"));
}

function sendPendingCommand(commandId, mode = "manual") {
  const command = state.pendingCommands.find((item) => item.id === commandId);
  if (!command) return;
  if (!canUseSocket()) {
    state.pendingCommands = markCommandQueued(state.pendingCommands, commandId, new Date());
    persistSurfaceState();
    state.status = "Command saved locally. It will send after reconnect and history sync.";
    return;
  }
  const requestId = nextRequestId();
  const payload = commandRequestPayload(command, requestId);
  if (!payload) return;
  state.pendingCommands = markCommandSending(state.pendingCommands, commandId, requestId, new Date());
  persistSurfaceState();
  try {
    sendRaw(payload);
    state.status = mode === "auto" ? "Sending queued command after resync..." : "Sending command...";
  } catch (error) {
    state.pendingCommands = markCommandFailed(state.pendingCommands, commandId, error.message || String(error), new Date());
    persistSurfaceState();
    setError(error.message || String(error));
  }
}

function sendDraft() {
  const text = state.draft.trim();
  if (!text) return;
  const command = createPendingMessageCommand({ content: text, sessionId: state.sessionId, now: new Date(), randomFn: Math.random });
  state.pendingCommands = appendPendingCommand(state.pendingCommands, command);
  state.draft = "";
  persistSurfaceState();
  if (canUseSocket() && state.phase !== "resyncing") {
    sendPendingCommand(command.id, "manual");
  } else {
    state.status = "Command queued locally. It will send after reconnect and history sync.";
    if (!canUseSocket() && activeCredential() && state.networkOnline !== false && state.pageVisible !== false) scheduleReconnect("queued command", true);
  }
}

function cancelTurn() {
  sendControl({ type: "cancel", id: nextRequestId() }, "Cancel");
}

function switchCredential(event) {
  state.activeId = event.target.value;
  const credential = activeCredential();
  if (credential) {
    state.host = credential.host;
    state.port = String(credential.port);
  }
  persistSurfaceState();
  scheduleReconnect("server switch", true);
}

function forgetActiveCredential() {
  const credential = activeCredential();
  if (!credential) return;
  disconnect();
  const next = state.credentials.filter((existing) => credentialId(existing) !== credentialId(credential));
  state.credentials = next;
  state.activeId = next.length ? credentialId(next[next.length - 1]) : "";
  saveCredentials(next);
  persistSurfaceState();
}

function requestHistorySync() {
  if (!canUseSocket()) {
    state.status = "History sync queued until reconnect.";
    scheduleReconnect("manual history sync", true);
    return;
  }
  try {
    setPhase("resyncing", "Resyncing history...");
    sendRaw({ type: "get_history", id: nextRequestId() });
  } catch (error) {
    setError(error.message || String(error));
  }
}

function retryPendingCommand(commandId) {
  state.pendingCommands = markCommandQueued(state.pendingCommands, commandId, new Date());
  persistSurfaceState();
  if (canUseSocket() && state.phase !== "resyncing") {
    sendPendingCommand(commandId, "manual");
  } else {
    state.status = "Command queued locally. It will send after reconnect and history sync.";
    scheduleReconnect("retry pending command", true);
  }
}

function restorePendingCommand(commandId) {
  const command = state.pendingCommands.find((item) => item.id === commandId);
  if (!command) return;
  state.draft = command.payload.content;
  state.pendingCommands = removePendingCommand(state.pendingCommands, commandId);
  persistSurfaceState();
  requestAnimationFrame(() => {
    const composer = document.getElementById("composer-input");
    if (composer) composer.focus();
  });
}

function discardPendingCommand(commandId) {
  state.pendingCommands = removePendingCommand(state.pendingCommands, commandId);
  persistSurfaceState();
  state.status = "Pending command discarded locally.";
}

function sendControl(payload, label) {
  if (!canUseSocket()) {
    state.status = `${label} needs a live connection. Reconnecting first...`;
    scheduleReconnect(label, true);
    return;
  }
  try {
    sendRaw(payload);
    state.status = `${label} requested`;
  } catch (error) {
    setError(error.message || String(error));
  }
}

function resumeSession(id) {
  state.sessionId = id;
  persistSurfaceState();
  sendControl({
    type: "resume_session",
    id: nextRequestId(),
    session_id: id,
    client_instance_id: deviceId(),
    client_has_local_history: state.transcript.length > 0,
    allow_session_takeover: true,
  }, "Session switch");
}

function setModel(model) {
  sendControl({ type: "set_model", id: nextRequestId(), model }, "Model switch");
}

function foregroundResync(reason) {
  state.pageVisible = documentIsVisible();
  persistSurfaceState();
  if (state.pageVisible === false) {
    clearReconnectTimer();
    setPhase("offline", "Backgrounded. Local state saved; will reconnect and resync on foreground.");
    return;
  }
  if (canUseSocket()) {
    setPhase("resyncing", `Foreground return. Resyncing after ${reason}...`);
    resyncAfterReconnect(reason);
  } else {
    scheduleReconnect(reason, true);
  }
}

function handleVisibilityChange() {
  state.pageVisible = documentIsVisible();
  persistSurfaceState();
  if (state.pageVisible) {
    foregroundResync("visibilitychange");
  } else {
    clearReconnectTimer();
    setPhase("offline", "Backgrounded. Local state saved; will reconnect and resync on foreground.");
  }
}

function handlePageShow() {
  state.pageVisible = documentIsVisible();
  foregroundResync("pageshow");
}

function handlePageHide() {
  state.pageVisible = false;
  clearReconnectTimer();
  persistSurfaceState();
}

function handleOnline() {
  state.networkOnline = true;
  foregroundResync("network online");
}

function handleOffline() {
  state.networkOnline = false;
  clearReconnectTimer();
  setPhase("offline", "Offline. Drafts and pending commands are saved locally.");
  persistSurfaceState();
}

function handleAck(id) {
  const result = applyCommandAck(state.pendingCommands, id);
  if (result.ackedCommand) {
    state.pendingCommands = result.commands;
    persistSurfaceState();
    appendEntry({ role: "user", text: result.ackedCommand.payload.content });
    state.status = state.pendingCommands.length ? pendingCommandSummary(state.pendingCommands) : "Sent";
  }
}

function handleLine(line) {
  let event;
  try {
    event = JSON.parse(line);
  } catch {
    state.status = `Ignored non-JSON frame: ${line.slice(0, 40)}`;
    return;
  }
  switch (event.type) {
    case "ack":
      handleAck(event.id);
      break;
    case "history":
      applyHistory(event);
      break;
    case "session":
      state.sessionId = event.session_id || state.sessionId;
      persistSurfaceState();
      break;
    case "session_renamed":
      state.sessionTitle = event.display_title || state.sessionTitle;
      break;
    case "state":
      state.sessionId = event.session_id || state.sessionId;
      persistSurfaceState();
      break;
    case "available_models_updated":
      state.availableModels = event.available_models || [];
      state.providerModel = event.provider_model || state.providerModel;
      break;
    case "model_changed":
      state.providerModel = event.model || state.providerModel;
      if (event.error) setError(event.error);
      break;
    case "tokens":
      state.tokens = formatTokens(event);
      break;
    case "status_detail":
      state.status = event.detail || event.message || event.status || JSON.stringify(event);
      break;
    case "notification":
      appendEntry({ role: "system", text: `${event.from_name || "jcode"}: ${event.message || ""}` });
      break;
    case "reasoning_delta":
      ensureAssistant().reasoning += event.text || "";
      break;
    case "reasoning_done":
      break;
    case "text_delta":
      ensureAssistant().text += event.text || "";
      break;
    case "text_replace":
      ensureAssistant().text = event.text || "";
      break;
    case "tool_start":
      currentToolId = event.id || `tool-${Date.now()}`;
      ensureAssistant().tools.push({ id: currentToolId, name: event.name || "tool", input: "", output: "", status: "input" });
      break;
    case "tool_input":
      toolById(currentToolId).input += event.delta || "";
      break;
    case "tool_exec":
      toolById(event.id || currentToolId).status = "running";
      break;
    case "tool_done": {
      const tool = toolById(event.id || currentToolId);
      tool.status = event.error ? "failed" : "done";
      tool.output = event.error || event.output || "";
      break;
    }
    case "message_end":
      currentAssistantId = null;
      currentToolId = null;
      break;
    case "done":
      currentAssistantId = null;
      currentToolId = null;
      state.status = "Ready";
      break;
    case "interrupted":
      currentAssistantId = null;
      currentToolId = null;
      state.status = "Interrupted";
      break;
    case "error":
      setError(event.message || "Server error");
      break;
    default:
      state.status = unknownProtocolEventStatus(event);
  }
}

function applyHistory(event) {
  state.sessionId = event.session_id || "";
  state.sessionTitle = event.display_title || "";
  state.providerName = event.provider_name || "";
  state.providerModel = event.provider_model || "";
  state.availableModels = event.available_models || [];
  state.allSessions = event.all_sessions || [];
  state.tokens = Array.isArray(event.total_tokens) ? `in ${event.total_tokens[0]} / out ${event.total_tokens[1]}` : "";
  state.transcript = (event.messages || []).map((message, index) => ({
    id: `hist-${index}`,
    role: normalizeRole(message.role),
    text: message.content || "",
    reasoning: "",
    tools: message.tool_data ? [historyTool(message.tool_data)] : [],
  }));
  currentAssistantId = null;
  currentToolId = null;
  finishHistoryResync();
}

function historyTool(tool) {
  return {
    id: tool.id || `tool-${Date.now()}`,
    name: tool.name || "tool",
    input: tool.input || "",
    output: tool.error || tool.output || "",
    status: tool.error ? "failed" : "done",
  };
}

function normalizeRole(role) {
  return role === "user" || role === "assistant" ? role : "system";
}

function appendEntry(entry) {
  state.transcript.push({
    id: `entry-${Date.now()}-${Math.random().toString(16).slice(2)}`,
    role: entry.role,
    text: entry.text || "",
    reasoning: entry.reasoning || "",
    tools: entry.tools || [],
  });
}

function ensureAssistant() {
  if (currentAssistantId) {
    const existing = state.transcript.find((entry) => entry.id === currentAssistantId);
    if (existing) return existing;
  }
  const entry = { id: `assistant-${Date.now()}-${Math.random().toString(16).slice(2)}`, role: "assistant", text: "", reasoning: "", tools: [] };
  state.transcript.push(entry);
  currentAssistantId = entry.id;
  return entry;
}

function toolById(id) {
  const entry = ensureAssistant();
  let tool = entry.tools.find((item) => item.id === id);
  if (!tool) {
    tool = { id: id || `tool-${Date.now()}`, name: "tool", input: "", output: "", status: "running" };
    entry.tools.push(tool);
  }
  return tool;
}

function formatTokens(event) {
  const input = event.input_tokens !== undefined ? event.input_tokens : (event.input !== undefined ? event.input : event.total_input_tokens);
  const output = event.output_tokens !== undefined ? event.output_tokens : (event.output !== undefined ? event.output : event.total_output_tokens);
  if (input !== undefined || output !== undefined) return `in ${input || 0} / out ${output || 0}`;
  return "";
}

function shortId(value) {
  if (!value) return "none";
  const text = String(value);
  return text.length > 14 ? `${text.slice(0, 6)}…${text.slice(-6)}` : text;
}

function visibleSessions() {
  const filter = state.sessionFilter.trim().toLowerCase();
  const sessions = state.allSessions || [];
  return filter ? sessions.filter((id) => String(id).toLowerCase().includes(filter)) : sessions;
}

function visibleModels() {
  const filter = state.modelFilter.trim().toLowerCase();
  const models = state.availableModels || [];
  return (filter ? models.filter((model) => String(model).toLowerCase().includes(filter)) : models).slice(0, 36);
}

function transcriptStats() {
  const entries = state.transcript || [];
  let tools = 0;
  let running = 0;
  entries.forEach((entry) => {
    (entry.tools || []).forEach((tool) => {
      tools += 1;
      if (tool.status === "running" || tool.status === "input") running += 1;
    });
  });
  return { entries: entries.length, tools, running };
}

function applyQuickPrompt(text) {
  setDraft(text);
  requestAnimationFrame(() => {
    const composer = document.getElementById("composer-input");
    if (composer) composer.focus();
  });
}

function submitOnEnter(event) {
  if (event.key === "Enter" && !event.shiftKey) {
    event.preventDefault();
    sendDraft();
  }
}

watch(
  () => {
    if (!state.transcript.length) return "";
    return state.transcript[state.transcript.length - 1].text || "";
  },
  () => requestAnimationFrame(() => {
    const bottom = document.getElementById("bottom");
    if (bottom) bottom.scrollIntoView({ block: "end" });
  })
);

const CredentialPicker = () => html`
  <div class="field small">
    <span>Server</span>
    <div class="server-readout">
      ${() => {
        const credential = activeCredential();
        return credential ? `${credential.serverName || "jcode"} · ${credential.host}:${credential.port}` : "manual pairing";
      }}
    </div>
  </div>
`;

const PairPanel = () => html`
  <section class="card pair-card">
    <div class="section-title">
      <h2>Pair to workstation</h2>
      <p>Run <code>jcode pair</code>, then enter the host and 6 digit code.</p>
    </div>
    <div class="grid two">
      <label class="field">
        <span>Host</span>
        <input autocomplete="hostname" autocapitalize="none" spellcheck="false" placeholder="devbox.tailnet.ts.net" .value="${() => state.host}" @input="${(event) => { state.host = event.target.value; }}" />
      </label>
      <label class="field port-field">
        <span>Port</span>
        <input inputmode="numeric" placeholder="7643" .value="${() => state.port}" @input="${(event) => { state.port = event.target.value; }}" />
      </label>
    </div>
    <div class="grid two">
      <label class="field">
        <span>Pairing code</span>
        <input inputmode="numeric" autocomplete="one-time-code" placeholder="123456" .value="${() => state.code}" @input="${(event) => { state.code = event.target.value; }}" />
      </label>
      <label class="field">
        <span>Device name</span>
        <input placeholder="Lenovo Y700" .value="${() => state.deviceName}" @input="${(event) => { state.deviceName = event.target.value; }}" />
      </label>
    </div>
    <div class="actions">
      <button class="primary" disabled="${() => canPair() && !state.pairing ? false : ""}" @click="${pair}">${() => state.pairing ? "Pairing..." : "Pair"}</button>
      <button disabled="${() => activeCredential() ? false : ""}" @click="${() => connect("manual")}">Connect saved</button>
      <button class="danger ghost" disabled="${() => activeCredential() ? false : ""}" @click="${forgetActiveCredential}">Forget</button>
    </div>
  </section>
`;

const Header = () => html`
  <header class="topbar">
    <div class="brand-block">
      <p class="eyebrow">jcode away cockpit</p>
      <h1>${() => { const credential = activeCredential(); return state.sessionTitle || (credential && credential.serverName) || "jcode"; }}</h1>
      <p class="meta">${() => [state.providerName, state.providerModel, state.tokens].filter(Boolean).join(" · ") || "Gateway client for Android / browser"}</p>
    </div>
    <div class="top-actions">
      <button class="ghost mode-toggle" @click="${() => { state.focusMode = !state.focusMode; persistSurfaceState(); }}">${() => state.focusMode ? "Cockpit" : "Focus"}</button>
      <div class="status" data-phase="${() => state.phase}">${() => phaseLabel(state.phase)}</div>
    </div>
  </header>
`;

const MetricsRail = () => html`
  <section class="metrics-rail" aria-label="jcode status metrics">
    <div class="metric live" data-phase="${() => state.phase}"><span>link</span><strong>${() => phaseLabel(state.phase)}</strong></div>
    <div class="metric"><span>session</span><strong>${() => shortId(state.sessionId)}</strong></div>
    <div class="metric"><span>stream</span><strong>${() => { const stats = transcriptStats(); return stats.running ? `${stats.running} active` : "idle"; }}</strong></div>
    <div class="metric"><span>turns</span><strong>${() => transcriptStats().entries}</strong></div>
    <div class="metric"><span>pending</span><strong>${() => state.pendingCommands.length ? pendingCommandSummary(state.pendingCommands) : `${transcriptStats().tools} tools`}</strong></div>
  </section>
`;

const QuickDeck = () => html`
  <section class="quick-deck" aria-label="quick prompts">
    <button @click="${() => applyQuickPrompt("Summarize the current session state, blockers, and next best action.")}">situational brief</button>
    <button @click="${() => applyQuickPrompt("Inspect the repo state, identify risks, and propose the highest leverage next step.")}">repo scan</button>
    <button @click="${() => applyQuickPrompt("Continue autonomously. Validate your work and report only important progress.")}">keep going</button>
    <button @click="${() => applyQuickPrompt("Show me changed files and what each change accomplishes.")}">diff brief</button>
  </section>
`;

const TranscriptEntry = (entry) => html`
  <article class="entry" data-role="${entry.role}">
    <div class="role">${entry.role}</div>
    ${() => entry.reasoning ? html`<pre class="reasoning">${entry.reasoning}</pre>` : ""}
    ${() => entry.tools.map((tool) => html`
      <details class="tool" open="${() => tool.status === "running" ? "" : false}">
        <summary>${tool.name} <span>${tool.status}</span></summary>
        ${tool.input ? html`<pre>${tool.input}</pre>` : ""}
        ${tool.output ? html`<pre>${tool.output}</pre>` : ""}
      </details>
    `.key(tool.id))}
    ${() => entry.text ? html`<pre class="message">${entry.text}</pre>` : html`<span class="muted">streaming...</span>`}
  </article>
`.key(entry.id);

const PendingCommandsPanel = () => html`
  <div class="pending-panel" role="status" aria-label="pending local commands">
    <div class="pending-head">
      <strong>${() => pendingCommandSummary(state.pendingCommands)}</strong>
      <span>Saved locally. Queued commands auto-send after reconnect + history sync; needs-review commands require a tap to avoid duplicates.</span>
    </div>
    ${() => state.pendingCommands.map((command) => html`
      <article class="pending-command" data-status="${command.status}">
        <div class="pending-meta">
          <span>${command.verb}</span>
          <strong>${command.status}</strong>
        </div>
        <pre>${command.payload.content}</pre>
        ${() => command.last_error ? html`<p class="pending-error">${command.last_error}</p>` : ""}
        <div class="actions compact">
          <button disabled="${() => command.status === "sending" ? "" : false}" @click="${() => retryPendingCommand(command.id)}">${() => command.status === "unknown" ? "Send anyway" : "Retry"}</button>
          <button class="ghost" @click="${() => restorePendingCommand(command.id)}">Edit draft</button>
          <button class="danger ghost" @click="${() => discardPendingCommand(command.id)}">Discard</button>
        </div>
      </article>
    `.key(command.id))}
  </div>
`;

const ChatPanel = () => html`
  <section class="card chat-card">
    <div class="chat-header">
      <div>
        <strong>${() => state.status}</strong>
        <p>${() => state.sessionId ? `session ${state.sessionId}` : "No session yet"}</p>
      </div>
      <div class="actions compact">
        <button disabled="${() => canUseSocket() ? false : ""}" @click="${cancelTurn}">Cancel</button>
        <button class="ghost" disabled="${() => activeCredential() ? false : ""}" @click="${requestHistorySync}">Sync</button>
        <button class="ghost" @click="${() => disconnect()}">Disconnect</button>
      </div>
    </div>
    ${() => state.error ? html`<div class="error">${state.error}</div>` : ""}
    ${() => state.pendingCommands.length ? PendingCommandsPanel() : ""}
    <div class="transcript">
      ${() => state.transcript.length ? state.transcript.map(TranscriptEntry) : html`<div class="empty">Pair, connect, then send a prompt.</div>`}
      <div id="bottom"></div>
    </div>
    <form class="composer" @submit="${(event) => { event.preventDefault(); sendDraft(); }}">
      <textarea id="composer-input" rows="3" placeholder="Ask jcode..." .value="${() => state.draft}" @input="${(event) => setDraft(event.target.value)}" @keydown="${submitOnEnter}"></textarea>
      <button class="primary" disabled="${() => state.draft.trim() ? false : ""}" type="submit">${() => canUseSocket() && state.phase !== "resyncing" ? "Send" : "Queue"}</button>
    </form>
  </section>
`;

const SessionPanel = () => html`
  <section class="card side-card">
    ${CredentialPicker()}
    <div class="side-section command-center">
      <div class="section-title tight">
        <h2>Pulse</h2>
        <p>${() => state.status}</p>
      </div>
      <div class="pulse-line"><span>model</span><strong>${() => state.providerModel || "unset"}</strong></div>
      <div class="pulse-line"><span>tokens</span><strong>${() => state.tokens || "no totals"}</strong></div>
      <div class="pulse-line"><span>server</span><strong>${() => { const credential = activeCredential(); return credential ? `${credential.host}:${credential.port}` : "manual"; }}</strong></div>
    </div>
    <div class="section-title tight">
      <h2>Sessions</h2>
      <p>${() => state.allSessions.length ? `${state.allSessions.length} reported by server` : "History sync will fill this."}</p>
    </div>
    <input class="filter-input" placeholder="filter sessions" .value="${() => state.sessionFilter}" @input="${(event) => { state.sessionFilter = event.target.value; persistSurfaceState(); }}" />
    <div class="session-list">
      ${() => visibleSessions().map((id) => html`
        <button class="session-chip" data-active="${() => id === state.sessionId ? "true" : "false"}" @click="${() => resumeSession(id)}">${id}</button>
      `.key(id))}
    </div>
    <div class="section-title tight">
      <h2>Models</h2>
      <p>${() => state.availableModels.length ? "Tap to switch" : "Server has not sent a model list yet."}</p>
    </div>
    <input class="filter-input" placeholder="filter models" .value="${() => state.modelFilter}" @input="${(event) => { state.modelFilter = event.target.value; persistSurfaceState(); }}" />
    <div class="session-list">
      ${() => visibleModels().map((model) => html`
        <button class="session-chip" data-active="${() => model === state.providerModel ? "true" : "false"}" @click="${() => setModel(model)}">${model}</button>
      `.key(model))}
    </div>
  </section>
`;

const App = () => html`
  <div class="shell" data-focus="${() => state.focusMode ? "true" : "false"}">
    ${Header()}
    ${MetricsRail()}
    ${QuickDeck()}
    <div class="layout">
      <div class="main-col">
        ${PairPanel()}
        ${ChatPanel()}
      </div>
      ${SessionPanel()}
    </div>
    <footer>
      Serve this folder over HTTP. Pairing uses <code>POST /pair</code>; browser WebSockets use <code>WS /ws?token=...</code> because browsers cannot set Authorization headers.
    </footer>
  </div>
`;

App()(document.getElementById("app"));

window.addEventListener("visibilitychange", handleVisibilityChange);
window.addEventListener("pageshow", handlePageShow);
window.addEventListener("pagehide", handlePageHide);
window.addEventListener("online", handleOnline);
window.addEventListener("offline", handleOffline);

persistSurfaceState();
if (activeCredential() && state.networkOnline && state.pageVisible) {
  scheduleReconnect("saved workstation", true);
}
