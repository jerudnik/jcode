use jcode_storage::{external_secret, tag};
use std::io::Write;

fn main() {
    let mut path = external_secret::<tag::ExternalClaudeCredentials>(()).unwrap();
    path.write_all(b"secret").unwrap();
}
