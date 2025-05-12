use sp1_helper::{build_program_with_args, BuildArgs};

fn main() {
    println!("cargo:rerun-if-changed=zkprogram/src");
    build_program_with_args(
        "./zkprogram",
        BuildArgs {
            features: vec!["sp1".to_string()],
            ..Default::default()
        },
    );
    println!("cargo:rerun-if-changed=crash_game/src");
    build_program_with_args(
        "./crash_game",
        BuildArgs {
            features: vec!["sp1".to_string()],
            ..Default::default()
        },
    );
}
