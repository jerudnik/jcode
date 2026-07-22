#[derive(Clone)]
pub struct ContextSnapshot {
    pub info: Option<crate::prompt::ContextInfo>,
    pub revision: u64,
    pub fresh: bool,
}
