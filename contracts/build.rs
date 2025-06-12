#[cfg(feature = "nobuild")]
fn main() {}
#[cfg(not(feature = "nobuild"))]
fn main() {
    use sp1_helper::{build_program_with_args, BuildArgs};
    println!("cargo:rerun-if-changed=board_game/src");
    build_program_with_args(
        "./board_game",
        BuildArgs {
            features: vec!["sp1".to_string()],
            output_directory: Some("../elf".to_string()),
            ..Default::default()
        },
    );
    println!("cargo:rerun-if-changed=crash_game/src");
    build_program_with_args(
        "./crash_game",
        BuildArgs {
            features: vec!["sp1".to_string()],
            output_directory: Some("../elf".to_string()),
            ..Default::default()
        },
    );
}
