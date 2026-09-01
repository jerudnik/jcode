use jcode_storage::{durable_path, tag};

fn main() {
    let _ = durable_path::<tag::SessionInboxItem>("nested/session".to_owned());
}
