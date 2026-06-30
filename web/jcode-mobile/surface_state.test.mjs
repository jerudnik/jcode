import test from "node:test";
import assert from "node:assert/strict";

import {
  applyCommandAck,
  appendPendingCommand,
  buildResyncRequests,
  classifySocketClose,
  commandRequestPayload,
  computeReconnectDelay,
  createPendingMessageCommand,
  isAuthFailureClose,
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

const fixedNow = new Date("2026-06-30T18:00:00.000Z");

function pending(content) {
  return createPendingMessageCommand({
    id: "cmd_test",
    content,
    sessionId: "ses_1",
    now: fixedNow,
    randomFn: () => 0.5,
  });
}

test("draft, active session, and pending commands survive local state recovery", () => {
  const command = pending("  continue validation  ");
  const raw = serializeSurfaceState({
    activeId: "host:7643",
    draft: "half typed prompt",
    sessionId: "ses_1",
    focusMode: true,
    sessionFilter: "ses",
    modelFilter: "sonnet",
    lastSyncAt: "2026-06-30T18:01:00.000Z",
    pendingCommands: [command],
  });

  const restored = restoreSurfaceState(raw);
  assert.equal(restored.activeId, "host:7643");
  assert.equal(restored.draft, "half typed prompt");
  assert.equal(restored.sessionId, "ses_1");
  assert.equal(restored.focusMode, true);
  assert.equal(restored.pendingCommands.length, 1);
  assert.equal(restored.pendingCommands[0].payload.content, "continue validation");
});

test("pending message commands are persisted before send and become request payloads", () => {
  const command = pending("ship it");
  const commands = appendPendingCommand([], command);
  const sending = markCommandSending(commands, command.id, 42, fixedNow);
  const payload = commandRequestPayload(sending[0], 42);

  assert.equal(commands[0].status, "queued");
  assert.equal(sending[0].status, "sending");
  assert.equal(sending[0].attempts, 1);
  assert.deepEqual(payload, { type: "message", id: 42, content: "ship it" });
});

test("ack reconciliation removes only the matching pending command", () => {
  const first = markCommandSending([pending("one")], "cmd_test", 7, fixedNow)[0];
  const second = Object.assign({}, pending("two"), { id: "cmd_second" });
  const result = applyCommandAck([first, second], 7);

  assert.equal(result.ackedCommand.id, "cmd_test");
  assert.equal(result.commands.length, 1);
  assert.equal(result.commands[0].id, "cmd_second");
});

test("close while backgrounded does not schedule reconnect and marks sending commands unknown", () => {
  const sending = markCommandSending([pending("background send")], "cmd_test", 9, fixedNow);
  const unknown = markInflightCommandsUnknown(sending, fixedNow, "background close");
  const close = classifySocketClose({ manual: false, online: true, visible: false, code: 1006, reason: "" });

  assert.equal(unknown[0].status, "unknown");
  assert.equal(unknown[0].request_id, null);
  assert.match(unknown[0].last_error, /background close/);
  assert.equal(close.phase, "offline");
  assert.equal(close.reconnect, false);
  assert.match(close.status, /foreground/);
});

test("foreground resync builds subscribe before get_history with session takeover hints", () => {
  const resync = buildResyncRequests({
    nextId: 5,
    sessionId: "ses_active",
    clientInstanceId: "surface-1",
    hasLocalHistory: true,
  });

  assert.equal(resync.nextId, 7);
  assert.equal(resync.requests[0].type, "subscribe");
  assert.equal(resync.requests[0].id, 5);
  assert.equal(resync.requests[0].target_session_id, "ses_active");
  assert.equal(resync.requests[0].client_instance_id, "surface-1");
  assert.equal(resync.requests[0].client_has_local_history, true);
  assert.equal(resync.requests[0].allow_session_takeover, true);
  assert.deepEqual(resync.requests[1], { type: "get_history", id: 6 });
});

test("offline, reconnecting, and auth failure lifecycle decisions are distinct", () => {
  const offline = classifySocketClose({ manual: false, online: false, visible: true, code: 1006, reason: "" });
  const reconnect = classifySocketClose({ manual: false, online: true, visible: true, code: 1006, reason: "" });
  const auth = classifySocketClose({ manual: false, online: true, visible: true, code: 1008, reason: "bad token" });

  assert.equal(offline.phase, "offline");
  assert.equal(offline.reconnect, false);
  assert.equal(reconnect.phase, "reconnecting");
  assert.equal(reconnect.reconnect, true);
  assert.equal(auth.phase, "auth_failure");
  assert.equal(auth.reconnect, false);
  assert.equal(isAuthFailureClose(4401, ""), true);
  assert.equal(phaseLabel("idle"), "idle session");
});

test("reconnect delay uses capped exponential backoff with deterministic jitter", () => {
  assert.equal(computeReconnectDelay(0, () => 0), 750);
  assert.equal(computeReconnectDelay(1, () => 0), 1500);
  assert.equal(computeReconnectDelay(2, () => 1), 3750);
  assert.equal(computeReconnectDelay(20, () => 1), 15000);
});

test("failed or unknown commands can be queued again or removed", () => {
  const sending = markCommandSending([pending("retry me")], "cmd_test", 3, fixedNow);
  const unknown = markInflightCommandsUnknown(sending, fixedNow, "lost ack");
  const queued = markCommandQueued(unknown, "cmd_test", fixedNow);
  const removed = removePendingCommand(queued, "cmd_test");

  assert.equal(pendingCommandSummary(unknown), "1 needs review");
  assert.equal(queued[0].status, "queued");
  assert.equal(queued[0].request_id, null);
  assert.equal(removed.length, 0);
});

test("stale resync acks cannot clear recovered unknown commands", () => {
  const sending = markCommandSending([pending("maybe sent")], "cmd_test", 1, fixedNow);
  const unknown = markInflightCommandsUnknown(sending, fixedNow, "reload");
  const result = applyCommandAck(unknown, 1);

  assert.equal(result.ackedCommand, null);
  assert.equal(result.commands.length, 1);
  assert.equal(result.commands[0].status, "unknown");
  assert.equal(result.commands[0].request_id, null);
});

test("unknown protocol events produce a non-blocking ignored status", () => {
  assert.equal(unknownProtocolEventStatus({ type: "future_surface_event" }), "Ignored future_surface_event event");
  assert.equal(unknownProtocolEventStatus({ type: "text_delta" }), "");
  assert.equal(unknownProtocolEventStatus({}), "Ignored unknown event");
});

test("bad JSON recovery falls back to an empty durable surface state", () => {
  const restored = restoreSurfaceState("not json");
  assert.equal(restored.draft, "");
  assert.equal(restored.pendingCommands.length, 0);
});
