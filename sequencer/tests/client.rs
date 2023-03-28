use assert_cmd::{assert::Assert, Command};
use serde::de::DeserializeOwned;

#[test]
fn deploy_fibonacci() {
    let output = client_command(&["declare", "cairo_programs/fibonacci.json"]);
    output.unwrap(); // if exit code was not successful, this will panic

    let output = client_command(&["deploy-account", "class_hash"]);
    output.unwrap();

    // todo: attempt invoke
    // todo: decide whether:
    //    - CLI returns class_hash on check_tx,
    //    - deliver_tx indexes class hash for a specific tx and creates an event for it,
    //    - deliver_tx indexes class hash for a specific tx and we are able to use the `query` hook,
    //
}

fn client_command(args: &[&str]) -> Result<serde_json::Value, String> {
    let command = &mut Command::cargo_bin("cli").unwrap();

    command
        .args(args)
        .assert()
        .try_success()
        .map(parse_output)
        .map_err(|e| e.to_string())
}

/// Extract the command assert output and deserialize it as json
fn parse_output<T: DeserializeOwned>(result: Assert) -> T {
    let output = String::from_utf8(result.get_output().stdout.to_vec()).unwrap();
    serde_json::from_str(&output).unwrap()
}
