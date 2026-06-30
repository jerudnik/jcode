import test from "node:test";
import assert from "node:assert/strict";

import {
  appendCommandLog,
  commandLogSummary,
  createCommandEnvelope,
  markCommandLogStatus,
  parseCommandInput,
  restoreCommandLog,
  serializeCommandLog,
} from "./surface_commands.mjs";

const fixedNow = new Date("2026-06-30T19:00:00.000Z");

function envelope(input) {
  const parsed = parseCommandInput(input, { sessionId: "sess_1" });
  assert.equal(parsed.ok, true, parsed.error || "parse ok");
  return createCommandEnvelope(parsed, { id: "cmd_test", now: fixedNow, randomFn: () => 0.5, sessionId: "sess_1" });
}

test("plain text and slash aliases parse to typed command envelopes", () => {
  const plain = parseCommandInput("ship the slice", { sessionId: "sess_1" });
  assert.equal(plain.verb, "message.send");
  assert.equal(plain.payload.content, "ship the slice");
  assert.equal(plain.payload.session_id, "sess_1");

  const card = parseCommandInput("/card Fix palette | acceptance stays visible", {});
  assert.equal(card.verb, "card.create");
  assert.equal(card.payload.title, "Fix palette");
  assert.equal(card.payload.body, "acceptance stays visible");

  const move = parseCommandInput("/move obj_1 done", {});
  assert.equal(move.verb, "card.move");
  assert.equal(move.payload.card_id, "obj_1");
  assert.equal(move.payload.status, "done");
});

test("json payloads and invalid verbs are safe", () => {
  const annotation = parseCommandInput('/annotation.create {"body":"note","target":{"kind":"file","uri":"repo://a"}}', {});
  assert.equal(annotation.ok, true);
  assert.equal(annotation.payload.target.uri, "repo://a");

  const cardCreate = parseCommandInput('/card create {"title":"Smoke card","status":"todo"}', {});
  assert.equal(cardCreate.ok, true);
  assert.equal(cardCreate.verb, "card.create");
  assert.equal(cardCreate.payload.title, "Smoke card");

  const cardMove = parseCommandInput('/card move obj_2 doing', {});
  assert.equal(cardMove.ok, true);
  assert.equal(cardMove.verb, "card.move");
  assert.equal(cardMove.payload.card_id, "obj_2");
  assert.equal(cardMove.payload.status, "doing");

  const artifactOpen = parseCommandInput('/artifact open crates/jcode/src/main.rs', {});
  assert.equal(artifactOpen.ok, true);
  assert.equal(artifactOpen.verb, "artifact.open");
  assert.equal(artifactOpen.payload.path, "crates/jcode/src/main.rs");

  const invalid = parseCommandInput("/rm -rf", {});
  assert.equal(invalid.ok, false);
  assert.match(invalid.error, /Unknown command/);
});

test("command log persists, recovers, and tracks statuses", () => {
  const first = envelope("/card Write test");
  const second = Object.assign({}, envelope("/intent capture this"), { id: "cmd_second" });
  const logged = appendCommandLog(appendCommandLog([], first), second);
  const sending = markCommandLogStatus(logged, "cmd_test", "sending", "", 7, fixedNow);
  const acked = markCommandLogStatus(sending, "cmd_test", "acked", "", 7, fixedNow);
  const failed = markCommandLogStatus(acked, "cmd_second", "failed", "bad target", null, fixedNow);
  const restored = restoreCommandLog(serializeCommandLog(failed));

  assert.equal(restored.length, 2);
  assert.equal(restored[0].status, "acked");
  assert.equal(restored[1].status, "failed");
  assert.equal(restored[1].error, "bad target");
  assert.equal(commandLogSummary(restored), "1 failed / 2 logged");
});

test("corrupt command log recovers empty", () => {
  assert.deepEqual(restoreCommandLog("not-json"), []);
});
