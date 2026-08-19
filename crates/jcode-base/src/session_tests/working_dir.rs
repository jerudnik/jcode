use super::*;

#[test]
fn new_session_records_created_working_dir_provenance() {
    let session = Session::create_with_id("session_working_dir_created".to_string(), None, None);

    assert_eq!(session.working_dir_set_by, Some(WorkingDirSetBy::Created));
    assert!(session.working_dir_set_at.is_some());
}

#[test]
fn session_json_without_working_dir_provenance_still_loads() -> anyhow::Result<()> {
    let session = Session::create_with_id("session_working_dir_legacy".to_string(), None, None);
    let mut json = serde_json::to_value(session)?;
    let object = json
        .as_object_mut()
        .expect("session serializes as an object");
    object.remove("working_dir_set_by");
    object.remove("working_dir_set_at");

    let loaded: Session = serde_json::from_value(json)?;
    assert_eq!(loaded.working_dir_set_by, None);
    assert_eq!(loaded.working_dir_set_at, None);
    Ok(())
}
