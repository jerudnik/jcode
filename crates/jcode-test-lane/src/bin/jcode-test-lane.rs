use jcode_test_lane::{
    Guard, LaneError, LaneOptions, acquire, lane_is_held, lock_path_from_env, read_holder,
    timeout_from_env,
};
use std::process::{Command, ExitStatus};

const EX_TEMPFAIL: i32 = 75;

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("run") => run_command(args.collect()),
        Some("status") if args.next().is_none() => print_status(),
        _ => {
            eprintln!(
                "usage: jcode-test-lane run [--label LABEL] [--timeout SECONDS] -- COMMAND [ARGS...]\n       jcode-test-lane status"
            );
            2
        }
    }
}

fn run_command(args: Vec<String>) -> i32 {
    let (label, timeout, command) = match parse_run_args(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("workspace test lane: {message}");
            return 2;
        }
    };
    let options = match LaneOptions::from_env(label, timeout) {
        Ok(options) => options,
        Err(err) => return report_lane_error(err),
    };
    let guard = match acquire(options) {
        Ok(guard) => guard,
        Err(err) => return report_lane_error(err),
    };

    let mut child = Command::new(&command[0]);
    child.args(&command[1..]);
    match &guard {
        Guard::Held(_) => {
            child.env("JCODE_TEST_LANE_HELD", std::process::id().to_string());
        }
        Guard::Nested => {}
        Guard::Bypassed => {
            child.env("JCODE_TEST_LANE_HELD", "bypassed");
        }
        Guard::Unsupported => {
            child.env("JCODE_TEST_LANE_HELD", "unsupported");
        }
    }
    let status = match child.status() {
        Ok(status) => status,
        Err(err) => {
            eprintln!(
                "workspace test lane: could not start command {:?}: {err}",
                command[0]
            );
            return 1;
        }
    };
    exit_code(status)
}

fn parse_run_args(
    args: Vec<String>,
) -> Result<(String, Option<std::time::Duration>, Vec<String>), String> {
    let mut label = "workspace-test".to_string();
    let mut timeout = timeout_from_env().map_err(|err| err.to_string())?;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--" => {
                let command = args[index + 1..].to_vec();
                if command.is_empty() {
                    return Err("missing command after --".to_string());
                }
                return Ok((label, timeout, command));
            }
            "--label" => {
                index += 1;
                label = args.get(index).ok_or("--label requires a value")?.clone();
            }
            "--timeout" => {
                index += 1;
                let value = args.get(index).ok_or("--timeout requires a value")?;
                timeout = jcode_test_lane::parse_timeout(value).map_err(|err| err.to_string())?;
            }
            other => {
                return Err(format!(
                    "unknown argument {other:?}; expected -- before command"
                ));
            }
        }
        index += 1;
    }
    Err("missing -- COMMAND".to_string())
}

fn print_status() -> i32 {
    let path = match lock_path_from_env() {
        Ok(path) => path,
        Err(err) => return report_lane_error(err),
    };
    match lane_is_held(&path) {
        Ok(false) => {
            println!("free");
            0
        }
        Ok(true) => {
            match read_holder(&path) {
                Some(holder) => println!("held by {holder}"),
                None => println!("held by an unknown process"),
            }
            0
        }
        Err(err) => report_lane_error(err),
    }
}

fn report_lane_error(err: LaneError) -> i32 {
    let is_timeout = matches!(err, LaneError::Timeout { .. });
    eprintln!("{err}");
    if is_timeout { EX_TEMPFAIL } else { 1 }
}

fn exit_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.signal().map_or(1, |signal| 128 + signal)
    }
    #[cfg(not(unix))]
    1
}
