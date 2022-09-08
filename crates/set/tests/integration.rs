//! Simply run the external bash script `resources/tests/test.bash`, and check for its success.

use std::process::Command;

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn integration_test() {
  // We run a bash script, so will not work on windows!
  let output = Command::new("bash")
    .arg("-c")
    .arg("cd resources/tests && ./test.bash")
    .output()
    .expect("failed to execute process");
  // eprintln!("Stderr: {:?}", String::from_utf8(output.stderr));
  assert!(output.status.success(), "Output status: {}", output.status);
  // We double check to be sure that we reach the end of the script
  assert_eq!(output.stdout, b"Everything seems OK :)\n");
}