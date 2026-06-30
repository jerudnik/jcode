const SCHEMA_VERSION = 1;
const DEFAULT_WORKSPACE_ID = "sw_local_default";
const MAX_OPS = 5000;

const DEFAULT_STATUSES = ["todo", "in_progress", "blocked", "done"];
const OBJECT_KINDS = ["card", "doc", "annotation", "intent", "artifact_ref"];
const OP_KINDS = [
  "workspace.create",
  "object.create",
  "object.update",
  "object.delete_soft",
  "object.restore",
  "body.update",
  "link.create",
  "link.delete",
  "view.create",
  "view.update",
];

function coerceString(value) {
  return typeof value === "string" ? value : "";
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function nowIso(now) {
  if (now instanceof Date) return now.toISOString();
  if (typeof now === "number") return new Date(now).toISOString();
  return new Date().toISOString();
}

export function makeSurfaceId(prefix, now, randomFn) {
  const timestamp = typeof now === "number" ? now : Date.now();
  const random = typeof randomFn === "function" ? randomFn() : Math.random();
  const suffix = Math.floor(random * 0xffffff).toString(16).padStart(6, "0");
  return `${prefix}_${timestamp.toString(36)}_${suffix}`;
}

function normalizeKind(kind) {
  return OBJECT_KINDS.indexOf(kind) >= 0 ? kind : "card";
}

function normalizeStatus(status, fallback) {
  const value = coerceString(status) || fallback || "open";
  return value;
}

function normalizeTags(tags) {
  return Array.isArray(tags) ? tags.map(coerceString).filter(Boolean).slice(0, 24) : [];
}

function normalizeTargets(targets) {
  if (!Array.isArray(targets)) return [];
  return targets
    .filter((target) => target && typeof target === "object")
    .map((target) => ({
      kind: coerceString(target.kind) || "workspace",
      uri: coerceString(target.uri),
      selector: target.selector && typeof target.selector === "object" ? target.selector : { type: "whole_resource" },
      fallback: target.fallback && typeof target.fallback === "object" ? target.fallback : {},
    }));
}

function normalizeLinks(links) {
  if (!Array.isArray(links)) return [];
  return links
    .filter((link) => link && typeof link === "object")
    .map((link) => ({ kind: coerceString(link.kind) || "relates_to", to: coerceString(link.to) }))
    .filter((link) => link.to);
}

export function defaultViews() {
  return [
    {
      view_id: "view_board_default",
      kind: "board",
      layout: { statuses: DEFAULT_STATUSES.slice() },
    },
    {
      view_id: "view_y700_landscape",
      kind: "command_plane",
      layout: {
        panes: [
          { id: "sessions", width: "1/3", order: 1 },
          { id: "chat", width: "1/3", order: 2 },
          { id: "artifact", width: "1/3", order: 3 },
        ],
        allowed_widths: ["1/3", "2/3", "full"],
      },
    },
    {
      view_id: "view_desktop_review",
      kind: "review",
      layout: { panes: ["workspace", "artifact", "annotations"] },
    },
  ];
}

export function defaultWorkspace(options) {
  const now = nowIso(options && options.now);
  const workspaceId = options && options.workspaceId ? options.workspaceId : DEFAULT_WORKSPACE_ID;
  return {
    schema_version: SCHEMA_VERSION,
    workspace_id: workspaceId,
    title: options && options.title ? options.title : "jcode surface workspace",
    scope: options && options.scope ? options.scope : { kind: "browser-local", root: "" },
    created_at: now,
    updated_at: now,
    active_view_id: "view_board_default",
  };
}

export function defaultWorkspaceState(options) {
  return {
    schema_version: SCHEMA_VERSION,
    workspace: defaultWorkspace(options || {}),
    objects: [],
    views: defaultViews(),
    bodies: {},
    ops: [],
    recovery: { snapshot_recovered: false, ops_recovered: false, bodies_recovered: false },
  };
}

export function normalizeObject(raw, now) {
  const createdAt = coerceString(raw && raw.created_at) || nowIso(now);
  const kind = normalizeKind(raw && raw.kind);
  const defaultStatus = kind === "card" ? "todo" : kind === "intent" ? "captured" : "open";
  return {
    id: coerceString(raw && raw.id) || makeSurfaceId("obj", Date.now(), Math.random),
    kind,
    title: coerceString(raw && raw.title) || "Untitled",
    status: normalizeStatus(raw && raw.status, defaultStatus),
    priority: coerceString(raw && raw.priority) || "normal",
    body_ref: coerceString(raw && raw.body_ref),
    created_at: createdAt,
    updated_at: coerceString(raw && raw.updated_at) || createdAt,
    created_by: coerceString(raw && raw.created_by) || "user",
    tags: normalizeTags(raw && raw.tags),
    targets: normalizeTargets(raw && raw.targets),
    links: normalizeLinks(raw && raw.links),
    fields: raw && raw.fields && typeof raw.fields === "object" ? raw.fields : {},
    deleted: raw && raw.deleted === true,
  };
}

export function normalizeWorkspaceState(raw, options) {
  const base = defaultWorkspaceState(options || {});
  if (!raw || typeof raw !== "object") return base;
  const workspace = raw.workspace && typeof raw.workspace === "object" ? raw.workspace : raw;
  base.workspace = Object.assign({}, base.workspace, {
    schema_version: SCHEMA_VERSION,
    workspace_id: coerceString(workspace.workspace_id) || base.workspace.workspace_id,
    title: coerceString(workspace.title) || base.workspace.title,
    scope: workspace.scope && typeof workspace.scope === "object" ? workspace.scope : base.workspace.scope,
    created_at: coerceString(workspace.created_at) || base.workspace.created_at,
    updated_at: coerceString(workspace.updated_at) || base.workspace.updated_at,
    active_view_id: coerceString(workspace.active_view_id) || base.workspace.active_view_id,
  });
  base.objects = Array.isArray(raw.objects) ? raw.objects.map((object) => normalizeObject(object, options && options.now)) : [];
  base.views = Array.isArray(raw.views) && raw.views.length ? raw.views : base.views;
  base.bodies = raw.bodies && typeof raw.bodies === "object" ? raw.bodies : {};
  base.ops = normalizeOps(raw.ops || []);
  return base;
}

export function storageKeys(workspaceId) {
  const id = workspaceId || DEFAULT_WORKSPACE_ID;
  return {
    snapshot: `jcode.surface.workspace.${id}.snapshot`,
    ops: `jcode.surface.workspace.${id}.ops`,
    bodies: `jcode.surface.workspace.${id}.bodies`,
  };
}

function parseJsonWithRecovery(raw, fallback) {
  if (!raw) return { value: fallback, recovered: false };
  try {
    return { value: JSON.parse(raw), recovered: false };
  } catch {
    return { value: fallback, recovered: true };
  }
}

export function loadWorkspaceState(storage, workspaceId, options) {
  const keys = storageKeys(workspaceId);
  const snapshotParsed = parseJsonWithRecovery(storage.getItem(keys.snapshot), null);
  const opsParsed = parseJsonWithRecovery(storage.getItem(keys.ops), []);
  const bodiesParsed = parseJsonWithRecovery(storage.getItem(keys.bodies), {});
  const state = normalizeWorkspaceState(snapshotParsed.value, Object.assign({}, options || {}, { workspaceId: workspaceId || DEFAULT_WORKSPACE_ID }));
  state.bodies = bodiesParsed.value && typeof bodiesParsed.value === "object" ? bodiesParsed.value : {};
  state.ops = normalizeOps(opsParsed.value);
  state.recovery = {
    snapshot_recovered: snapshotParsed.recovered,
    ops_recovered: opsParsed.recovered,
    bodies_recovered: bodiesParsed.recovered,
  };
  if (state.ops.length) return replayOperations(Object.assign({}, state, { ops: [] }), state.ops, { keepOps: true });
  return state;
}

export function saveWorkspaceState(storage, state) {
  const normalized = normalizeWorkspaceState(state, { workspaceId: state.workspace.workspace_id });
  const keys = storageKeys(normalized.workspace.workspace_id);
  const snapshot = {
    schema_version: SCHEMA_VERSION,
    workspace: normalized.workspace,
    objects: normalized.objects,
    views: normalized.views,
  };
  storage.setItem(keys.snapshot, JSON.stringify(snapshot));
  storage.setItem(keys.ops, JSON.stringify(normalized.ops.slice(-MAX_OPS)));
  storage.setItem(keys.bodies, JSON.stringify(normalized.bodies));
  return normalized;
}

export function compactWorkspaceState(storage, state) {
  const next = normalizeWorkspaceState(state, { workspaceId: state.workspace.workspace_id });
  next.ops = [];
  next.workspace.updated_at = nowIso();
  saveWorkspaceState(storage, next);
  return next;
}

function normalizeOps(ops) {
  if (!Array.isArray(ops)) return [];
  return ops
    .filter((op) => op && typeof op === "object" && OP_KINDS.indexOf(op.kind) >= 0)
    .slice(-MAX_OPS)
    .map((op) => Object.assign({}, op, {
      op_id: coerceString(op.op_id) || makeSurfaceId("op", Date.now(), Math.random),
      workspace_id: coerceString(op.workspace_id) || DEFAULT_WORKSPACE_ID,
      created_at: coerceString(op.created_at) || nowIso(),
      source: op.source && typeof op.source === "object" ? op.source : { surface_id: "surface_browser" },
    }));
}

export function createOperation(kind, payload, options) {
  const now = options && options.now ? options.now : new Date();
  return Object.assign({
    op_id: options && options.opId ? options.opId : makeSurfaceId("op", now instanceof Date ? now.getTime() : Date.now(), options && options.randomFn),
    workspace_id: options && options.workspaceId ? options.workspaceId : DEFAULT_WORKSPACE_ID,
    kind,
    created_at: nowIso(now),
    source: { surface_id: options && options.surfaceId ? options.surfaceId : "surface_browser" },
  }, payload || {});
}

export function applyOperation(state, op, options) {
  const next = normalizeWorkspaceState(state, { workspaceId: state.workspace.workspace_id });
  const now = coerceString(op.created_at) || nowIso(options && options.now);
  if (op.kind === "workspace.create") {
    next.workspace = Object.assign({}, next.workspace, op.workspace || {}, { updated_at: now });
  }
  if (op.kind === "object.create") {
    const object = normalizeObject(Object.assign({}, op.object || {}, { updated_at: now }), now);
    if (!object.body_ref) object.body_ref = `body/${object.id}.md`;
    next.objects = next.objects.filter((existing) => existing.id !== object.id).concat([object]);
    if (typeof op.body === "string") next.bodies[object.body_ref] = op.body;
  }
  if (op.kind === "object.update") {
    next.objects = next.objects.map((object) => {
      if (object.id !== op.object_id) return object;
      const patch = op.patch && typeof op.patch === "object" ? op.patch : {};
      return normalizeObject(Object.assign({}, object, patch, { id: object.id, updated_at: now }), now);
    });
  }
  if (op.kind === "object.delete_soft") {
    next.objects = next.objects.map((object) => object.id === op.object_id ? Object.assign({}, object, { deleted: true, updated_at: now }) : object);
  }
  if (op.kind === "object.restore") {
    next.objects = next.objects.map((object) => object.id === op.object_id ? Object.assign({}, object, { deleted: false, updated_at: now }) : object);
  }
  if (op.kind === "body.update") {
    const object = next.objects.find((item) => item.id === op.object_id);
    const bodyRef = object && object.body_ref ? object.body_ref : op.body_ref;
    if (bodyRef) next.bodies[bodyRef] = coerceString(op.body);
    next.objects = next.objects.map((item) => item.id === op.object_id ? Object.assign({}, item, { updated_at: now }) : item);
  }
  if (op.kind === "link.create") {
    next.objects = next.objects.map((object) => {
      if (object.id !== op.object_id) return object;
      const link = { kind: coerceString(op.link && op.link.kind) || "relates_to", to: coerceString(op.link && op.link.to) };
      if (!link.to) return object;
      return Object.assign({}, object, { links: normalizeLinks(object.links.concat([link])), updated_at: now });
    });
  }
  if (op.kind === "link.delete") {
    next.objects = next.objects.map((object) => {
      if (object.id !== op.object_id) return object;
      return Object.assign({}, object, { links: object.links.filter((link) => !(link.to === op.to && link.kind === op.link_kind)), updated_at: now });
    });
  }
  if (op.kind === "view.create") {
    const view = op.view && typeof op.view === "object" ? op.view : null;
    if (view && view.view_id) next.views = next.views.filter((existing) => existing.view_id !== view.view_id).concat([view]);
  }
  if (op.kind === "view.update") {
    next.views = next.views.map((view) => view.view_id === op.view_id ? Object.assign({}, view, op.patch || {}) : view);
  }
  next.workspace.updated_at = now;
  return next;
}

export function appendOperation(state, op) {
  const applied = applyOperation(state, op);
  applied.ops = normalizeOps((state.ops || []).concat([op]));
  return applied;
}

export function replayOperations(state, ops, options) {
  let next = normalizeWorkspaceState(state, { workspaceId: state.workspace.workspace_id });
  const normalizedOps = normalizeOps(ops);
  normalizedOps.forEach((op) => {
    next = applyOperation(next, op);
  });
  next.ops = options && options.keepOps ? normalizedOps : next.ops;
  return next;
}

export function createObjectOperation(kind, fields, body, options) {
  const now = options && options.now ? options.now : new Date();
  const id = fields && fields.id ? fields.id : makeSurfaceId("obj", now instanceof Date ? now.getTime() : Date.now(), options && options.randomFn);
  const bodyRef = fields && fields.body_ref ? fields.body_ref : `body/${id}.md`;
  const object = Object.assign({}, fields || {}, {
    id,
    kind: normalizeKind(kind),
    body_ref: bodyRef,
    created_at: fields && fields.created_at ? fields.created_at : nowIso(now),
    updated_at: nowIso(now),
  });
  return createOperation("object.create", { object, body: coerceString(body) }, Object.assign({}, options || {}, { now }));
}

export function bodyForObject(state, object) {
  if (!object || !object.body_ref) return "";
  return coerceString(state.bodies[object.body_ref]);
}

function visibleObjects(state, kind) {
  return normalizeWorkspaceState(state, { workspaceId: state.workspace.workspace_id }).objects.filter((object) => object.kind === kind && object.deleted !== true);
}

export function boardProjection(state) {
  const statuses = DEFAULT_STATUSES.slice();
  const lanes = statuses.map((status) => ({ status, cards: [] }));
  visibleObjects(state, "card").forEach((card) => {
    let lane = lanes.find((item) => item.status === card.status);
    if (!lane) {
      lane = { status: card.status, cards: [] };
      lanes.push(lane);
    }
    lane.cards.push(card);
  });
  lanes.forEach((lane) => lane.cards.sort((a, b) => coerceString(a.fields.ordinal).localeCompare(coerceString(b.fields.ordinal)) || a.created_at.localeCompare(b.created_at)));
  return lanes;
}

export function docsProjection(state) {
  return visibleObjects(state, "doc").map((doc) => Object.assign({}, doc, { body: bodyForObject(state, doc) }));
}

export function annotationsProjection(state) {
  const groups = [];
  visibleObjects(state, "annotation").forEach((annotation) => {
    const target = annotation.targets[0] || { kind: "workspace", uri: "workspace" };
    const key = `${target.kind}:${target.uri || "workspace"}`;
    let group = groups.find((item) => item.key === key);
    if (!group) {
      group = { key, target, annotations: [] };
      groups.push(group);
    }
    group.annotations.push(Object.assign({}, annotation, { body: bodyForObject(state, annotation) }));
  });
  return groups;
}

export function intentInboxProjection(state) {
  return visibleObjects(state, "intent").filter((intent) => intent.status !== "routed" && intent.status !== "done").map((intent) => Object.assign({}, intent, { body: bodyForObject(state, intent) }));
}

export function artifactsProjection(state) {
  return visibleObjects(state, "artifact_ref").map((artifact) => Object.assign({}, artifact, { body: bodyForObject(state, artifact) }));
}

export function artifactReviewProjection(state, artifactId) {
  const artifacts = artifactsProjection(state);
  const artifact = artifacts.find((item) => item.id === artifactId) || artifacts[0] || null;
  if (!artifact) return { artifact: null, annotations: [], cards: [], docs: [] };
  const linked = visibleObjects(state, "annotation").filter((object) => object.links.some((link) => link.to === artifact.id) || object.targets.some((target) => target.uri === artifact.fields.path || target.uri === artifact.id));
  const cards = visibleObjects(state, "card").filter((object) => object.links.some((link) => link.to === artifact.id));
  const docs = visibleObjects(state, "doc").filter((object) => object.links.some((link) => link.to === artifact.id));
  return { artifact, annotations: linked, cards, docs };
}

export function workspaceCounts(state) {
  const normalized = normalizeWorkspaceState(state, { workspaceId: state.workspace.workspace_id });
  const counts = { card: 0, doc: 0, annotation: 0, intent: 0, artifact_ref: 0 };
  normalized.objects.forEach((object) => {
    if (object.deleted === true) return;
    counts[object.kind] = (counts[object.kind] || 0) + 1;
  });
  return counts;
}

export function createFixtureWorkspace(cardCount, annotationCount, options) {
  let state = defaultWorkspaceState(options || {});
  for (let index = 0; index < cardCount; index += 1) {
    const status = DEFAULT_STATUSES[index % DEFAULT_STATUSES.length];
    state = appendOperation(state, createObjectOperation("card", {
      id: `obj_card_${index}`,
      title: `Fixture card ${index}`,
      status,
      fields: { ordinal: String(index).padStart(5, "0") },
    }, `Fixture body ${index}`, options || {}));
  }
  for (let index = 0; index < annotationCount; index += 1) {
    state = appendOperation(state, createObjectOperation("annotation", {
      id: `obj_annotation_${index}`,
      title: `Fixture annotation ${index}`,
      status: "open",
      targets: [{ kind: "file_range", uri: `repo://fixture_${index % 10}.md`, selector: { type: "line_range", start: index, end: index + 1 }, fallback: { type: "text_quote", exact: "fixture" } }],
    }, `Annotation body ${index}`, options || {}));
  }
  return state;
}
