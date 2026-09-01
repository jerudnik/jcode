use jcode_storage::{secret_path, tag};

fn main() {
    let _ = secret_path::<tag::ConfigLock>(());
}
