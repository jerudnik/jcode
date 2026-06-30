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
import {
  appendCommandLog,
  commandLogSummary,
  createCommandEnvelope,
  markCommandLogStatus,
  parseCommandInput,
  restoreCommandLog,
  serializeCommandLog,
} from "./surface_commands.mjs";
import {
  annotationsProjection,
  appendOperation,
  artifactReviewProjection,
  artifactsProjection,
  boardProjection,
  bodyForObject,
  compactWorkspaceState,
  createObjectOperation,
  createOperation,
  docsProjection,
  intentInboxProjection,
  loadWorkspaceState,
  saveWorkspaceState,
  workspaceCounts,
} from "./surface_workspace_store.mjs";

const STORAGE_KEY = "jcode.mobileWeb.credentials.v1";
const DEVICE_ID_KEY = "jcode.mobileWeb.deviceId.v1";
const SURFACE_STATE_KEY = "jcode.mobileWeb.surfaceState.v1";
const COMMAND_LOG_KEY = "jcode.surface.commandLog.v1";
const WORKSPACE_ID = "sw_local_default";

const persistedSurface = loadSurfaceState();
const persistedCommandLog = loadCommandLog();
const persistedWorkspace = loadWorkspace();

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
  pendingCommandCount: persistedSurface.pendingCommands.length,
  commandInput: "",
  commandError: "",
  commandLog: persistedCommandLog,
  workspace: persistedWorkspace,
  workspaceView: "board",
  workspaceTitleDraft: "",
  workspaceBodyDraft: "",
  annotationDraft: "",
  intentDraft: persistedSurface.draft && persistedSurface.draft.startsWith("/intent") ? persistedSurface.draft : "",
  artifactDraft: "",
  metaInstruction: "Review the selected workspace objects and propose the next best action.",
  selectedObjectId: "",
  selectedArtifactId: "",
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
  replacePendingCommands(markInflightCommandsUnknown(state.pendingCommands, new Date(), "Recovered after reload before ack"));
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

function loadCommandLog() {
  try {
    return restoreCommandLog(localStorage.getItem(COMMAND_LOG_KEY));
  } catch {
    return [];
  }
}

function persistCommandLog() {
  try {
    localStorage.setItem(COMMAND_LOG_KEY, serializeCommandLog(state.commandLog));
  } catch {
    state.status = "Command log storage unavailable; command replay may not survive reload.";
  }
}

function replaceCommandLog(commands) {
  state.commandLog = commands;
  persistCommandLog();
}

function loadWorkspace() {
  try {
    return loadWorkspaceState(localStorage, WORKSPACE_ID, { workspaceId: WORKSPACE_ID });
  } catch {
    return loadWorkspaceState({ getItem: () => null, setItem: () => {} }, WORKSPACE_ID, { workspaceId: WORKSPACE_ID });
  }
}

function persistWorkspace() {
  try {
    saveWorkspaceState(localStorage, state.workspace);
  } catch {
    state.status = "Workspace storage unavailable; local objects may not survive reload.";
  }
}

function replaceWorkspace(nextWorkspace) {
  state.workspace = nextWorkspace;
  persistWorkspace();
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

function replacePendingCommands(commands) {
  state.pendingCommands.splice(0, state.pendingCommands.length, ...commands);
  state.pendingCommandCount = state.pendingCommands.length;
}

function markLoggedCommand(commandId, status, error, requestId) {
  replaceCommandLog(markCommandLogStatus(state.commandLog, commandId, status, error || "", requestId, new Date()));
}

function logCommand(envelope) {
  replaceCommandLog(appendCommandLog(state.commandLog, envelope));
}

function applyWorkspaceOperation(op) {
  replaceWorkspace(appendOperation(state.workspace, op));
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
    replacePendingCommands(markInflightCommandsUnknown(state.pendingCommands, new Date(), "Socket replaced before ack"));
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
    replacePendingCommands(markInflightCommandsUnknown(state.pendingCommands, new Date(), "Connection closed before ack"));
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
  replacePendingCommands(markInflightCommandsUnknown(state.pendingCommands, new Date(), "Disconnected before ack"));
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
    replacePendingCommands(markCommandQueued(state.pendingCommands, commandId, new Date()));
    persistSurfaceState();
    state.status = "Command saved locally. It will send after reconnect and history sync.";
    return;
  }
  const requestId = nextRequestId();
  const payload = commandRequestPayload(command, requestId);
  if (!payload) return;
  replacePendingCommands(markCommandSending(state.pendingCommands, commandId, requestId, new Date()));
  markLoggedCommand(commandId, "sending", "", requestId);
  persistSurfaceState();
  try {
    sendRaw(payload);
    state.status = mode === "auto" ? "Sending queued command after resync..." : "Sending command...";
  } catch (error) {
    replacePendingCommands(markCommandFailed(state.pendingCommands, commandId, error.message || String(error), new Date()));
    markLoggedCommand(commandId, "failed", error.message || String(error), requestId);
    persistSurfaceState();
    setError(error.message || String(error));
  }
}

function sendDraft() {
  const text = state.draft.trim();
  if (!text) return;
  const command = createPendingMessageCommand({ content: text, sessionId: state.sessionId, now: new Date(), randomFn: Math.random });
  const parsed = { verb: "message.send", payload: { session_id: state.sessionId, content: text }, raw: text };
  logCommand(createCommandEnvelope(parsed, { id: command.id, sessionId: state.sessionId, status: "pending" }));
  replacePendingCommands(appendPendingCommand(state.pendingCommands, command));
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
  replacePendingCommands(markCommandQueued(state.pendingCommands, commandId, new Date()));
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
  replacePendingCommands(removePendingCommand(state.pendingCommands, commandId));
  persistSurfaceState();
  requestAnimationFrame(() => {
    const composer = document.getElementById("composer-input");
    if (composer) composer.focus();
  });
}

function discardPendingCommand(commandId) {
  replacePendingCommands(removePendingCommand(state.pendingCommands, commandId));
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
    replacePendingCommands(result.commands);
    markLoggedCommand(result.ackedCommand.id, "acked", "", id);
    persistSurfaceState();
    appendEntry({ role: "user", text: result.ackedCommand.payload.content });
    state.status = state.pendingCommands.length ? pendingCommandSummary(state.pendingCommands) : "Sent";
  }
}

function workspaceOptions() {
  return { workspaceId: state.workspace.workspace.workspace_id, surfaceId: deviceId(), now: new Date(), randomFn: Math.random };
}

function createLocalObject(kind, fields, body) {
  const op = createObjectOperation(kind, fields, body || "", workspaceOptions());
  applyWorkspaceOperation(op);
  return op.object;
}

function executeWorkspaceCommand(envelope) {
  const payload = envelope.payload || {};
  if (envelope.verb === "card.create") {
    const card = createLocalObject("card", { title: payload.title || "Untitled card", status: payload.status || "todo", priority: payload.priority || "normal" }, payload.body || "");
    state.selectedObjectId = card.id;
    state.workspaceView = "board";
    markLoggedCommand(envelope.id, "acked", "", null);
    state.status = `Card created: ${card.title}`;
    return true;
  }
  if (envelope.verb === "card.move") {
    applyWorkspaceOperation(createOperation("object.update", { object_id: payload.card_id, patch: { status: payload.status || "todo" } }, workspaceOptions()));
    state.workspaceView = "board";
    markLoggedCommand(envelope.id, "acked", "", null);
    state.status = `Card moved to ${payload.status || "todo"}`;
    return true;
  }
  if (envelope.verb === "doc.create") {
    const doc = createLocalObject("doc", { title: payload.title || "Untitled doc", status: "open" }, payload.body || "");
    state.selectedObjectId = doc.id;
    state.workspaceView = "docs";
    markLoggedCommand(envelope.id, "acked", "", null);
    state.status = `Doc created: ${doc.title}`;
    return true;
  }
  if (envelope.verb === "annotation.create") {
    const annotation = createLocalObject("annotation", {
      title: payload.title || (payload.body || "Annotation").slice(0, 48),
      status: "open",
      targets: [payload.target || { kind: "workspace", uri: state.selectedArtifactId || "workspace" }],
      links: state.selectedArtifactId ? [{ kind: "annotates", to: state.selectedArtifactId }] : [],
    }, payload.body || "");
    state.selectedObjectId = annotation.id;
    state.workspaceView = "annotations";
    markLoggedCommand(envelope.id, "acked", "", null);
    state.status = "Annotation saved locally.";
    return true;
  }
  if (envelope.verb === "intent.capture") {
    const intent = createLocalObject("intent", { title: (payload.body || "Intent").slice(0, 48), status: "captured", priority: payload.urgency || "normal" }, payload.body || "");
    state.selectedObjectId = intent.id;
    state.workspaceView = "intents";
    markLoggedCommand(envelope.id, "acked", "", null);
    state.status = "Intent captured locally.";
    return true;
  }
  if (envelope.verb === "intent.route") {
    const source = state.workspace.objects.find((object) => object.id === payload.intent_id);
    const body = payload.body || (source ? bodyForObject(state.workspace, source) : "");
    const card = createLocalObject("card", { title: body.slice(0, 48) || "Routed intent", status: "todo", links: source ? [{ kind: "created_from", to: source.id }] : [] }, body);
    if (source) applyWorkspaceOperation(createOperation("object.update", { object_id: source.id, patch: { status: "routed" } }, workspaceOptions()));
    state.selectedObjectId = card.id;
    state.workspaceView = "board";
    markLoggedCommand(envelope.id, "acked", "", null);
    state.status = "Intent routed to card.";
    return true;
  }
  if (envelope.verb === "artifact.open") {
    const path = payload.path || payload.artifact_id || "artifact";
    const artifact = createLocalObject("artifact_ref", { title: path.split("/").pop() || path, status: "open", fields: { path } }, path);
    state.selectedArtifactId = artifact.id;
    state.workspaceView = "artifact";
    markLoggedCommand(envelope.id, "acked", "", null);
    state.status = `Artifact reference saved: ${path}`;
    return true;
  }
  if (envelope.verb === "summary.request" || envelope.verb === "agent.meta" || envelope.verb === "surface.handoff") {
    state.metaInstruction = payload.instruction || payload.context || payload.scope || state.metaInstruction;
    state.workspaceView = "meta";
    markLoggedCommand(envelope.id, "acked", "", null);
    state.status = "Meta-agent prompt prepared locally.";
    return true;
  }
  return false;
}

function executeCommandInput(input) {
  const parsed = parseCommandInput(input, { sessionId: state.sessionId });
  if (!parsed.ok) {
    state.commandError = parsed.error;
    state.status = parsed.error;
    return;
  }
  const envelope = createCommandEnvelope(parsed, { sessionId: state.sessionId, surfaceId: deviceId(), status: "pending" });
  logCommand(envelope);
  state.commandError = "";
  state.commandInput = "";
  if (parsed.verb === "message.send") {
    const command = createPendingMessageCommand({ id: envelope.id, content: parsed.payload.content, sessionId: state.sessionId, now: new Date(), randomFn: Math.random });
    replacePendingCommands(appendPendingCommand(state.pendingCommands, command));
    persistSurfaceState();
    if (canUseSocket() && state.phase !== "resyncing") sendPendingCommand(command.id, "manual");
    else state.status = "Command queued locally. It will send after reconnect and history sync.";
    return;
  }
  if (parsed.verb === "turn.cancel") {
    cancelTurn();
    markLoggedCommand(envelope.id, canUseSocket() ? "acked" : "failed", canUseSocket() ? "" : "needs live connection", null);
    return;
  }
  if (parsed.verb === "history.sync") {
    requestHistorySync();
    markLoggedCommand(envelope.id, "acked", "", null);
    return;
  }
  if (parsed.verb === "model.set") {
    setModel(parsed.payload.model);
    markLoggedCommand(envelope.id, "acked", "", null);
    return;
  }
  if (parsed.verb === "session.switch" || parsed.verb === "session.attach") {
    resumeSession(parsed.payload.session_id);
    markLoggedCommand(envelope.id, "acked", "", null);
    return;
  }
  if (executeWorkspaceCommand(envelope)) return;
  markLoggedCommand(envelope.id, "failed", "This verb is logged but not executable in the browser surface yet.", null);
  state.status = "Command logged safely; this verb needs a live orchestrator path.";
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

function submitCommandOnEnter(event) {
  if (event.key === "Enter" && !event.shiftKey) {
    event.preventDefault();
    executeCommandInput(state.commandInput);
  }
}

function setCommandShortcut(text) {
  state.commandInput = text;
  requestAnimationFrame(() => {
    const input = document.getElementById("command-input");
    if (input) input.focus();
  });
}

function createCardFromDraft() {
  const title = state.workspaceTitleDraft.trim() || "Untitled card";
  const body = state.workspaceBodyDraft.trim();
  executeCommandInput(`/card create ${JSON.stringify({ title, body, status: "todo" })}`);
  state.workspaceTitleDraft = "";
  state.workspaceBodyDraft = "";
}

function createDocFromDraft() {
  const title = state.workspaceTitleDraft.trim() || "Untitled doc";
  const body = state.workspaceBodyDraft.trim();
  executeCommandInput(`/doc create ${JSON.stringify({ title, body })}`);
  state.workspaceTitleDraft = "";
  state.workspaceBodyDraft = "";
}

function captureIntentFromDraft() {
  const body = state.intentDraft.trim();
  if (!body) return;
  executeCommandInput(`/intent ${body}`);
  state.intentDraft = "";
}

function annotateFromDraft() {
  const body = state.annotationDraft.trim();
  if (!body) return;
  executeCommandInput(`/annotate ${JSON.stringify({ body, target: { kind: "workspace", uri: state.selectedArtifactId || "workspace" } })}`);
  state.annotationDraft = "";
}

function openArtifactFromDraft() {
  const path = state.artifactDraft.trim();
  if (!path) return;
  executeCommandInput(`/artifact open ${JSON.stringify({ path })}`);
  state.artifactDraft = "";
}

function compactWorkspace() {
  replaceWorkspace(compactWorkspaceState(localStorage, state.workspace));
  state.status = "Workspace snapshot compacted locally.";
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
    <div class="metric"><span>pending</span><strong>${() => state.pendingCommandCount ? pendingCommandSummary(state.pendingCommands) : `${transcriptStats().tools} tools`}</strong></div>
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

const CommandPalette = () => html`
  <section class="card command-palette" aria-label="typed command palette">
    <div class="section-title">
      <h2>Command palette</h2>
      <p>Durable typed verbs. Commands are logged before execution, and message sends survive reload/reconnect.</p>
    </div>
    <form class="command-form" @submit="${(event) => { event.preventDefault(); executeCommandInput(state.commandInput); }}">
      <input id="command-input" autocapitalize="none" spellcheck="false" placeholder="/card create {&quot;title&quot;:&quot;Fix mobile flow&quot;}" .value="${() => state.commandInput}" @input="${(event) => { state.commandInput = event.target.value; }}" @keydown="${submitCommandOnEnter}" />
      <button class="primary" type="submit" disabled="${() => state.commandInput.trim() ? false : ""}">Run</button>
    </form>
    ${() => state.commandError ? html`<p class="pending-error command-error">${state.commandError}</p>` : ""}
    <div class="command-shortcuts">
      <button @click="${() => setCommandShortcut("/intent Capture follow-up from the away cockpit")}">intent</button>
      <button @click="${() => setCommandShortcut("/card create {\"title\":\"Review latest diff\",\"status\":\"todo\"}")}">card</button>
      <button @click="${() => setCommandShortcut("/doc create {\"title\":\"Session notes\",\"body\":\"Notes from mobile surface.\"}")}">doc</button>
      <button @click="${() => setCommandShortcut("/history sync")}">sync</button>
      <button @click="${() => setCommandShortcut("/summary request")}">meta</button>
    </div>
    <div class="command-log" aria-label="durable verb log">
      <div class="pending-head">
        <strong>${() => commandLogSummary(state.commandLog)}</strong>
        <span>Recent commands</span>
      </div>
      ${() => state.commandLog.slice(0, 8).map((command) => html`
        <article class="command-row" data-status="${command.status}">
          <span>${command.verb}</span>
          <strong>${command.status}</strong>
          <code>${shortId(command.id)}</code>
          ${command.error ? html`<small>${command.error}</small>` : ""}
        </article>
      `.key(command.id))}
    </div>
  </section>
`;

const WorkspaceTabs = () => {
  const tabs = [
    ["board", "Board"],
    ["docs", "Docs"],
    ["annotations", "Annotations"],
    ["intents", "Intents"],
    ["artifact", "Artifact"],
    ["meta", "Meta"],
  ];
  return html`<div class="workspace-tabs">${tabs.map(([id, label]) => html`<button data-active="${() => state.workspaceView === id ? "true" : "false"}" @click="${() => { state.workspaceView = id; }}">${label}</button>`)}</div>`;
};

const WorkspaceObject = (object, options = {}) => html`
  <article class="workspace-object" data-kind="${object.kind}" data-selected="${() => state.selectedObjectId === object.id ? "true" : "false"}" @click="${() => { state.selectedObjectId = object.id; }}">
    <div class="object-head">
      <strong>${object.title || object.id}</strong>
      <span>${object.status || object.kind}</span>
    </div>
    ${options.body ? html`<p>${options.body}</p>` : ""}
    <div class="object-meta"><code>${shortId(object.id)}</code><span>${object.updated_at || object.created_at}</span></div>
  </article>
`;

const BoardView = () => html`
  <div class="board-view">
    ${() => boardProjection(state.workspace).map((lane) => html`
      <section class="board-lane" data-lane="${lane.status}">
        <h3>${lane.status} <span>${lane.cards.length}</span></h3>
        ${() => lane.cards.length ? lane.cards.map((card) => html`
          <article class="workspace-object board-card" data-kind="card" @click="${() => { state.selectedObjectId = card.id; }}">
            <div class="object-head"><strong>${card.title}</strong><span>${card.fields.priority || "normal"}</span></div>
            <div class="actions compact lane-actions">
              <button @click="${(event) => { event.stopPropagation(); executeCommandInput(`/card move ${JSON.stringify({ card_id: card.id, status: "todo" })}`); }}">todo</button>
              <button @click="${(event) => { event.stopPropagation(); executeCommandInput(`/card move ${JSON.stringify({ card_id: card.id, status: "doing" })}`); }}">doing</button>
              <button @click="${(event) => { event.stopPropagation(); executeCommandInput(`/card move ${JSON.stringify({ card_id: card.id, status: "done" })}`); }}">done</button>
            </div>
          </article>
        `.key(card.id)) : html`<div class="empty mini">No cards</div>`}
      </section>
    `)}
  </div>
`;

const DocsView = () => html`
  <div class="object-list">${() => docsProjection(state.workspace).length ? docsProjection(state.workspace).map((doc) => WorkspaceObject(doc, { body: doc.body }).key(doc.id)) : html`<div class="empty mini">Create a doc from the form below.</div>`}</div>
`;

const AnnotationsView = () => html`
  <div class="object-list">${() => annotationsProjection(state.workspace).length ? annotationsProjection(state.workspace).map((group) => html`
    <section class="annotation-group">
      <h3>${group.key}</h3>
      ${group.annotations.map((annotation) => WorkspaceObject(annotation, { body: annotation.body }).key(annotation.id))}
    </section>
  `) : html`<div class="empty mini">No annotations yet.</div>`}</div>
`;

const IntentsView = () => html`
  <div class="object-list">${() => intentInboxProjection(state.workspace).length ? intentInboxProjection(state.workspace).map((intent) => html`
    <article class="workspace-object" data-kind="intent">
      <div class="object-head"><strong>${intent.title}</strong><span>${intent.status}</span></div>
      <p>${intent.body}</p>
      <div class="actions compact"><button @click="${() => executeCommandInput(`/intent route ${JSON.stringify({ intent_id: intent.id, body: intent.body })}`)}">Route to card</button></div>
    </article>
  `.key(intent.id)) : html`<div class="empty mini">Capture away-from-keyboard intents here.</div>`}</div>
`;

const ArtifactReviewView = () => html`
  <div class="artifact-review">
    <div class="artifact-list">
      ${() => artifactsProjection(state.workspace).length ? artifactsProjection(state.workspace).map((artifact) => html`<button data-active="${() => state.selectedArtifactId === artifact.id ? "true" : "false"}" @click="${() => { state.selectedArtifactId = artifact.id; }}">${artifact.title}</button>`.key(artifact.id)) : html`<div class="empty mini">Open an artifact path below.</div>`}
    </div>
    ${() => {
      const review = artifactReviewProjection(state.workspace, state.selectedArtifactId);
      return review.artifact ? html`
        <section class="artifact-detail">
          <h3>${review.artifact.title}</h3>
          <p>${review.artifact.body || review.artifact.fields.path || "artifact"}</p>
          <div class="review-counts"><span>${review.annotations.length} annotations</span><span>${review.cards.length} cards</span><span>${review.docs.length} docs</span></div>
        </section>
      ` : "";
    }}
  </div>
`;

const MetaAgentView = () => html`
  <div class="meta-builder">
    <label class="field">
      <span>Meta-agent prompt</span>
      <textarea rows="5" .value="${() => state.metaInstruction}" @input="${(event) => { state.metaInstruction = event.target.value; }}"></textarea>
    </label>
    <button class="primary" @click="${() => executeCommandInput(`/summary request ${JSON.stringify({ instruction: state.metaInstruction, counts: workspaceCounts(state.workspace) })}`)}">Prepare summary request</button>
  </div>
`;

const WorkspaceCreatePanel = () => html`
  <div class="workspace-create">
    <label class="field"><span>Title</span><input placeholder="Object title" .value="${() => state.workspaceTitleDraft}" @input="${(event) => { state.workspaceTitleDraft = event.target.value; }}" /></label>
    <label class="field"><span>Body</span><textarea rows="3" placeholder="Body, notes, or context" .value="${() => state.workspaceBodyDraft}" @input="${(event) => { state.workspaceBodyDraft = event.target.value; }}"></textarea></label>
    <div class="actions compact"><button @click="${createCardFromDraft}">Create card</button><button @click="${createDocFromDraft}">Create doc</button></div>
    <label class="field"><span>Intent</span><textarea rows="2" placeholder="Capture an intent before context is lost" .value="${() => state.intentDraft}" @input="${(event) => { state.intentDraft = event.target.value; }}"></textarea></label>
    <div class="actions compact"><button @click="${captureIntentFromDraft}">Capture intent</button></div>
    <label class="field"><span>Annotation</span><textarea rows="2" placeholder="Annotate selected artifact or workspace" .value="${() => state.annotationDraft}" @input="${(event) => { state.annotationDraft = event.target.value; }}"></textarea></label>
    <div class="actions compact"><button @click="${annotateFromDraft}">Save annotation</button></div>
    <label class="field"><span>Artifact path</span><input placeholder="crates/jcode/src/main.rs" .value="${() => state.artifactDraft}" @input="${(event) => { state.artifactDraft = event.target.value; }}" /></label>
    <div class="actions compact"><button @click="${openArtifactFromDraft}">Open artifact</button><button class="ghost" @click="${compactWorkspace}">Compact</button></div>
  </div>
`;

const WorkspacePanel = () => html`
  <section class="card workspace-card" aria-label="surface workspace">
    <div class="workspace-header">
      <div class="section-title">
        <h2>Surface workspace</h2>
        <p>${() => { const counts = workspaceCounts(state.workspace); return `${counts.card} cards · ${counts.doc} docs · ${counts.annotation} annotations · ${counts.intent} intents · ${counts.artifact_ref} artifacts`; }}</p>
      </div>
      ${WorkspaceTabs()}
    </div>
    <div class="workspace-grid">
      <div class="workspace-view">
        ${() => state.workspaceView === "board" ? BoardView() : ""}
        ${() => state.workspaceView === "docs" ? DocsView() : ""}
        ${() => state.workspaceView === "annotations" ? AnnotationsView() : ""}
        ${() => state.workspaceView === "intents" ? IntentsView() : ""}
        ${() => state.workspaceView === "artifact" ? ArtifactReviewView() : ""}
        ${() => state.workspaceView === "meta" ? MetaAgentView() : ""}
      </div>
      ${WorkspaceCreatePanel()}
    </div>
  </section>
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
    ${() => state.pendingCommandCount ? PendingCommandsPanel() : ""}
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
        ${CommandPalette()}
        ${ChatPanel()}
        ${WorkspacePanel()}
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
