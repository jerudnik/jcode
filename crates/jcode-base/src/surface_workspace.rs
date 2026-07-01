use anyhow::{Context, Result, bail};
use chrono::Utc;
use jcode_protocol::{
    SurfaceWorkspaceApplyResult, SurfaceWorkspaceExport, SurfaceWorkspaceObject,
    SurfaceWorkspaceOperation, SurfaceWorkspaceSnapshot,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

pub const SURFACE_WORKSPACE_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_WORKSPACE_ID: &str = "sw_local_default";

#[derive(Debug, Clone)]
pub struct SurfaceWorkspaceStore {
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapshotFile {
    schema_version: u32,
    workspace_id: String,
    title: String,
    updated_at: String,
    #[serde(default)]
    objects: Vec<SurfaceWorkspaceObject>,
    #[serde(default)]
    views: Vec<Value>,
}

impl SurfaceWorkspaceStore {
    pub fn from_jcode_home() -> Result<Self> {
        Ok(Self::new(surface_workspaces_dir()?))
    }

    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn open_or_create(
        &self,
        workspace_id: &str,
        title: Option<&str>,
    ) -> Result<SurfaceWorkspaceSnapshot> {
        let workspace_id = sanitize_workspace_id(workspace_id)?;
        if let Some(snapshot) = self.load_snapshot(&workspace_id)? {
            return Ok(snapshot);
        }
        let now = now_iso();
        let snapshot = SurfaceWorkspaceSnapshot {
            schema_version: SURFACE_WORKSPACE_SCHEMA_VERSION,
            workspace_id: workspace_id.clone(),
            title: title.unwrap_or("Surface workspace").to_string(),
            updated_at: now,
            objects: Vec::new(),
            views: default_views(),
            bodies: BTreeMap::new(),
        };
        self.save_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub fn get_snapshot(&self, workspace_id: &str) -> Result<SurfaceWorkspaceSnapshot> {
        let workspace_id = sanitize_workspace_id(workspace_id)?;
        if let Some(snapshot) = self.load_snapshot(&workspace_id)? {
            return Ok(snapshot);
        }
        self.open_or_create(&workspace_id, None)
    }

    pub fn export(&self, workspace_id: &str) -> Result<SurfaceWorkspaceExport> {
        let workspace_id = sanitize_workspace_id(workspace_id)?;
        Ok(SurfaceWorkspaceExport {
            snapshot: self.get_snapshot(&workspace_id)?,
            ops: self.load_ops(&workspace_id)?,
        })
    }

    pub fn apply_ops(
        &self,
        workspace_id: &str,
        ops: &[SurfaceWorkspaceOperation],
    ) -> Result<SurfaceWorkspaceApplyResult> {
        let workspace_id = sanitize_workspace_id(workspace_id)?;
        let mut snapshot = self.get_snapshot(&workspace_id)?;
        let mut applied = 0usize;
        for op in ops {
            if op.workspace_id != workspace_id {
                bail!(
                    "operation workspace_id {} does not match {}",
                    op.workspace_id,
                    workspace_id
                );
            }
            apply_operation(&mut snapshot, op)?;
            self.append_op(op)?;
            applied += 1;
        }
        snapshot.updated_at = now_iso();
        self.save_snapshot(&snapshot)?;
        Ok(SurfaceWorkspaceApplyResult {
            workspace_id,
            applied,
            snapshot,
        })
    }

    fn workspace_dir(&self, workspace_id: &str) -> PathBuf {
        self.root.join(workspace_id)
    }

    fn snapshot_path(&self, workspace_id: &str) -> PathBuf {
        self.workspace_dir(workspace_id).join("snapshot.json")
    }

    fn snapshot_bak_path(&self, workspace_id: &str) -> PathBuf {
        self.workspace_dir(workspace_id).join("snapshot.json.bak")
    }

    fn bodies_path(&self, workspace_id: &str) -> PathBuf {
        self.workspace_dir(workspace_id).join("bodies.json")
    }

    fn bodies_bak_path(&self, workspace_id: &str) -> PathBuf {
        self.workspace_dir(workspace_id).join("bodies.json.bak")
    }

    fn ops_path(&self, workspace_id: &str) -> PathBuf {
        self.workspace_dir(workspace_id).join("ops.jsonl")
    }

    fn load_snapshot(&self, workspace_id: &str) -> Result<Option<SurfaceWorkspaceSnapshot>> {
        let snapshot_path = self.snapshot_path(workspace_id);
        if !snapshot_path.exists() && !self.snapshot_bak_path(workspace_id).exists() {
            return Ok(None);
        }
        let snapshot: SnapshotFile =
            read_json_with_bak(&snapshot_path, &self.snapshot_bak_path(workspace_id))
                .with_context(|| format!("load surface workspace snapshot {}", workspace_id))?;
        let bodies: BTreeMap<String, String> = read_json_with_bak(
            &self.bodies_path(workspace_id),
            &self.bodies_bak_path(workspace_id),
        )
        .unwrap_or_default();
        Ok(Some(SurfaceWorkspaceSnapshot {
            schema_version: snapshot.schema_version,
            workspace_id: snapshot.workspace_id,
            title: snapshot.title,
            updated_at: snapshot.updated_at,
            objects: snapshot.objects,
            views: snapshot.views,
            bodies,
        }))
    }

    fn save_snapshot(&self, snapshot: &SurfaceWorkspaceSnapshot) -> Result<()> {
        let workspace_id = sanitize_workspace_id(&snapshot.workspace_id)?;
        fs::create_dir_all(self.workspace_dir(&workspace_id))?;
        let snapshot_file = SnapshotFile {
            schema_version: snapshot.schema_version,
            workspace_id: snapshot.workspace_id.clone(),
            title: snapshot.title.clone(),
            updated_at: snapshot.updated_at.clone(),
            objects: snapshot.objects.clone(),
            views: snapshot.views.clone(),
        };
        write_json_atomic(
            &self.snapshot_path(&workspace_id),
            &self.snapshot_bak_path(&workspace_id),
            &snapshot_file,
        )?;
        write_json_atomic(
            &self.bodies_path(&workspace_id),
            &self.bodies_bak_path(&workspace_id),
            &snapshot.bodies,
        )?;
        Ok(())
    }

    fn append_op(&self, op: &SurfaceWorkspaceOperation) -> Result<()> {
        let workspace_id = sanitize_workspace_id(&op.workspace_id)?;
        fs::create_dir_all(self.workspace_dir(&workspace_id))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.ops_path(&workspace_id))?;
        serde_json::to_writer(&mut file, op)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        Ok(())
    }

    pub fn load_ops(&self, workspace_id: &str) -> Result<Vec<SurfaceWorkspaceOperation>> {
        let workspace_id = sanitize_workspace_id(workspace_id)?;
        let path = self.ops_path(&workspace_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file = File::open(path)?;
        let mut ops = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            ops.push(serde_json::from_str(trimmed)?);
        }
        Ok(ops)
    }
}

pub fn surface_workspaces_dir() -> Result<PathBuf> {
    let home = if let Some(path) = std::env::var_os("JCODE_HOME") {
        PathBuf::from(path)
    } else {
        dirs::home_dir()
            .context("could not determine home directory")?
            .join(".jcode")
    };
    Ok(home.join("surface_workspaces"))
}

pub fn sanitize_workspace_id(workspace_id: &str) -> Result<String> {
    let trimmed = workspace_id.trim();
    if trimmed.is_empty() {
        return Ok(DEFAULT_WORKSPACE_ID.to_string());
    }
    if trimmed.len() > 120
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
    {
        bail!("invalid workspace id");
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    {
        bail!("invalid workspace id");
    }
    Ok(trimmed.to_string())
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn default_views() -> Vec<Value> {
    vec![
        json!({"id":"view_board","kind":"board","title":"Board"}),
        json!({"id":"view_docs","kind":"docs","title":"Docs"}),
        json!({"id":"view_annotations","kind":"annotations","title":"Annotations"}),
        json!({"id":"view_intents","kind":"intents","title":"Intent inbox"}),
        json!({"id":"view_artifacts","kind":"artifacts","title":"Artifact review"}),
    ]
}

fn apply_operation(
    snapshot: &mut SurfaceWorkspaceSnapshot,
    op: &SurfaceWorkspaceOperation,
) -> Result<()> {
    match op.kind.as_str() {
        "object.upsert" | "object.create" => {
            let object: SurfaceWorkspaceObject = serde_json::from_value(
                op.payload
                    .get("object")
                    .cloned()
                    .unwrap_or_else(|| op.payload.clone()),
            )?;
            upsert_object(snapshot, object);
            if let Some(body) = op.payload.get("body").and_then(Value::as_str)
                && let Some(id) = op
                    .payload
                    .get("object")
                    .and_then(|v| v.get("id"))
                    .and_then(Value::as_str)
                    .or_else(|| op.payload.get("id").and_then(Value::as_str))
            {
                snapshot.bodies.insert(id.to_string(), body.to_string());
            }
        }
        "object.update" => {
            let object_id = op
                .payload
                .get("object_id")
                .and_then(Value::as_str)
                .context("object.update needs object_id")?;
            let patch = op
                .payload
                .get("patch")
                .and_then(Value::as_object)
                .context("object.update needs patch")?;
            let object = snapshot
                .objects
                .iter_mut()
                .find(|object| object.id == object_id)
                .context("object not found")?;
            if let Some(title) = patch.get("title").and_then(Value::as_str) {
                object.title = title.to_string();
            }
            if let Some(status) = patch.get("status").and_then(Value::as_str) {
                object.status = status.to_string();
            }
            if let Some(kind) = patch.get("kind").and_then(Value::as_str) {
                object.kind = kind.to_string();
            }
            if let Some(fields) = patch.get("fields") {
                object.fields = fields.clone();
            }
            if let Some(targets) = patch.get("targets").and_then(Value::as_array) {
                object.targets = targets.clone();
            }
            if let Some(links) = patch.get("links").and_then(Value::as_array) {
                object.links = links.clone();
            }
            if let Some(deleted) = patch.get("deleted").and_then(Value::as_bool) {
                object.deleted = deleted;
            }
            object.updated_at = op.created_at.clone();
            if let Some(body) = patch.get("body").and_then(Value::as_str) {
                snapshot
                    .bodies
                    .insert(object_id.to_string(), body.to_string());
            }
        }
        "object.delete" => {
            let object_id = op
                .payload
                .get("object_id")
                .and_then(Value::as_str)
                .context("object.delete needs object_id")?;
            if let Some(object) = snapshot
                .objects
                .iter_mut()
                .find(|object| object.id == object_id)
            {
                object.deleted = true;
                object.updated_at = op.created_at.clone();
            }
        }
        "body.set" => {
            let object_id = op
                .payload
                .get("object_id")
                .and_then(Value::as_str)
                .context("body.set needs object_id")?;
            let body = op
                .payload
                .get("body")
                .and_then(Value::as_str)
                .unwrap_or_default();
            snapshot
                .bodies
                .insert(object_id.to_string(), body.to_string());
        }
        other => bail!("unsupported workspace operation kind: {other}"),
    }
    Ok(())
}

fn upsert_object(snapshot: &mut SurfaceWorkspaceSnapshot, object: SurfaceWorkspaceObject) {
    if let Some(existing) = snapshot
        .objects
        .iter_mut()
        .find(|existing| existing.id == object.id)
    {
        *existing = object;
    } else {
        snapshot.objects.push(object);
    }
}

fn read_json_with_bak<T: for<'de> Deserialize<'de>>(path: &Path, bak_path: &Path) -> Result<T> {
    match fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
    {
        Some(value) => Ok(value),
        None => {
            let text = fs::read_to_string(bak_path)?;
            Ok(serde_json::from_str(&text)?)
        }
    }
}

fn write_json_atomic<T: Serialize>(path: &Path, bak_path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if path.exists() {
        fs::copy(path, bak_path)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut file = File::create(&tmp)?;
        serde_json::to_writer_pretty(&mut file, value)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
    }
    fs::rename(tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_object(id: &str, title: &str) -> SurfaceWorkspaceObject {
        let now = "2026-06-30T20:00:00.000Z".to_string();
        SurfaceWorkspaceObject {
            id: id.to_string(),
            kind: "card".to_string(),
            title: title.to_string(),
            status: "todo".to_string(),
            fields: json!({"priority":"normal"}),
            targets: Vec::new(),
            links: Vec::new(),
            created_at: now.clone(),
            updated_at: now,
            deleted: false,
        }
    }

    fn sample_op(id: &str, title: &str) -> SurfaceWorkspaceOperation {
        SurfaceWorkspaceOperation {
            op_id: format!("op_{id}"),
            workspace_id: "sw_test".to_string(),
            kind: "object.upsert".to_string(),
            created_at: "2026-06-30T20:00:00.000Z".to_string(),
            source: json!({"surface_id":"test"}),
            payload: json!({"object": sample_object(id, title), "body": "body text"}),
        }
    }

    #[test]
    fn store_round_trips_snapshot_ops_and_export() {
        let temp = tempdir().unwrap();
        let store = SurfaceWorkspaceStore::new(temp.path());
        let opened = store
            .open_or_create("sw_test", Some("Test workspace"))
            .unwrap();
        assert_eq!(opened.workspace_id, "sw_test");

        let result = store
            .apply_ops("sw_test", &[sample_op("obj_1", "First card")])
            .unwrap();
        assert_eq!(result.applied, 1);
        assert_eq!(result.snapshot.objects.len(), 1);
        assert_eq!(result.snapshot.bodies.get("obj_1").unwrap(), "body text");

        let export = store.export("sw_test").unwrap();
        assert_eq!(export.ops.len(), 1);
        assert_eq!(export.snapshot.objects[0].title, "First card");
    }

    #[test]
    fn corrupt_snapshot_recovers_from_backup() {
        let temp = tempdir().unwrap();
        let store = SurfaceWorkspaceStore::new(temp.path());
        store
            .apply_ops("sw_test", &[sample_op("obj_1", "First card")])
            .unwrap();
        store
            .apply_ops("sw_test", &[sample_op("obj_2", "Second card")])
            .unwrap();
        fs::write(store.snapshot_path("sw_test"), "not json").unwrap();
        let snapshot = store.get_snapshot("sw_test").unwrap();
        assert!(snapshot.objects.iter().any(|object| object.id == "obj_1"));
    }

    #[test]
    fn invalid_workspace_ids_are_rejected() {
        assert!(sanitize_workspace_id("../bad").is_err());
        assert!(sanitize_workspace_id("bad/slash").is_err());
        assert_eq!(sanitize_workspace_id("").unwrap(), DEFAULT_WORKSPACE_ID);
    }
}
