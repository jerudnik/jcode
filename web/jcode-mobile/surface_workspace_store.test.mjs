import test from "node:test";
import assert from "node:assert/strict";

import {
  annotationsProjection,
  appendOperation,
  artifactReviewProjection,
  boardProjection,
  compactWorkspaceState,
  createFixtureWorkspace,
  createObjectOperation,
  createOperation,
  defaultWorkspaceState,
  docsProjection,
  intentInboxProjection,
  loadWorkspaceState,
  replayOperations,
  saveWorkspaceState,
  storageKeys,
  workspaceCounts,
} from "./surface_workspace_store.mjs";

function memoryStorage() {
  const data = new Map();
  return {
    getItem(key) {
      return data.has(key) ? data.get(key) : null;
    },
    setItem(key, value) {
      data.set(key, String(value));
    },
    removeItem(key) {
      data.delete(key);
    },
  };
}

const fixedNow = new Date("2026-06-30T19:05:00.000Z");
const idOptions = { now: fixedNow, randomFn: () => 0.5, workspaceId: "sw_test" };

test("workspace store supports object CRUD, bodies, and projections", () => {
  let state = defaultWorkspaceState({ workspaceId: "sw_test", now: fixedNow });
  state = appendOperation(state, createObjectOperation("card", { id: "obj_card", title: "Ship store", status: "todo" }, "card body", idOptions));
  state = appendOperation(state, createObjectOperation("doc", { id: "obj_doc", title: "Plan" }, "# Plan", idOptions));
  state = appendOperation(state, createObjectOperation("annotation", { id: "obj_note", title: "Note", targets: [{ kind: "file", uri: "repo://a.md" }] }, "clarify", idOptions));
  state = appendOperation(state, createObjectOperation("intent", { id: "obj_intent", title: "Captured", status: "captured" }, "route me", idOptions));
  state = appendOperation(state, createObjectOperation("artifact_ref", { id: "obj_artifact", title: "Diff", fields: { path: "repo://diff.patch" } }, "artifact", idOptions));
  state = appendOperation(state, createOperation("object.update", { object_id: "obj_card", patch: { status: "done" } }, idOptions));
  state = appendOperation(state, createOperation("link.create", { object_id: "obj_note", link: { kind: "annotates", to: "obj_artifact" } }, idOptions));

  assert.equal(workspaceCounts(state).card, 1);
  assert.equal(boardProjection(state).find((lane) => lane.status === "done").cards[0].title, "Ship store");
  assert.equal(docsProjection(state)[0].body, "# Plan");
  assert.equal(annotationsProjection(state)[0].annotations[0].body, "clarify");
  assert.equal(intentInboxProjection(state)[0].body, "route me");
  assert.equal(artifactReviewProjection(state, "obj_artifact").annotations[0].id, "obj_note");
});

test("operation replay and snapshot compaction round trip through localStorage keys", () => {
  const storage = memoryStorage();
  let state = defaultWorkspaceState({ workspaceId: "sw_test", now: fixedNow });
  const op = createObjectOperation("card", { id: "obj_card", title: "Replay me", status: "todo" }, "body", idOptions);
  state = appendOperation(state, op);
  saveWorkspaceState(storage, state);

  const loaded = loadWorkspaceState(storage, "sw_test", { now: fixedNow });
  assert.equal(loaded.objects[0].title, "Replay me");
  assert.equal(loaded.ops.length, 1);

  const compacted = compactWorkspaceState(storage, loaded);
  assert.equal(compacted.ops.length, 0);
  const afterCompact = loadWorkspaceState(storage, "sw_test", { now: fixedNow });
  assert.equal(afterCompact.objects[0].title, "Replay me");
  assert.equal(afterCompact.ops.length, 0);
});

test("replayOperations can rebuild from only an operation log", () => {
  const base = defaultWorkspaceState({ workspaceId: "sw_test", now: fixedNow });
  const ops = [
    createObjectOperation("card", { id: "obj_card", title: "From ops" }, "body", idOptions),
    createOperation("object.update", { object_id: "obj_card", patch: { status: "blocked" } }, idOptions),
  ];
  const rebuilt = replayOperations(base, ops, { keepOps: true });
  assert.equal(rebuilt.objects[0].status, "blocked");
  assert.equal(rebuilt.ops.length, 2);
});

test("corrupt localStorage recovers to a safe empty workspace", () => {
  const storage = memoryStorage();
  const keys = storageKeys("sw_test");
  storage.setItem(keys.snapshot, "not-json");
  storage.setItem(keys.ops, "also-bad");
  storage.setItem(keys.bodies, "bad-bodies");
  const recovered = loadWorkspaceState(storage, "sw_test", { now: fixedNow });
  assert.equal(recovered.objects.length, 0);
  assert.equal(recovered.recovery.snapshot_recovered, true);
  assert.equal(recovered.recovery.ops_recovered, true);
  assert.equal(recovered.recovery.bodies_recovered, true);
});

test("500 card and 1000 annotation fixture projections stay fast", () => {
  const state = createFixtureWorkspace(500, 1000, { workspaceId: "sw_perf", now: fixedNow, randomFn: () => 0.5 });
  const started = performance.now();
  const board = boardProjection(state);
  const annotations = annotationsProjection(state);
  const elapsed = performance.now() - started;

  assert.equal(workspaceCounts(state).card, 500);
  assert.equal(workspaceCounts(state).annotation, 1000);
  assert.equal(board.reduce((total, lane) => total + lane.cards.length, 0), 500);
  assert.equal(annotations.reduce((total, group) => total + group.annotations.length, 0), 1000);
  assert.ok(elapsed < 250, `projection fixture took ${elapsed}ms`);
});
