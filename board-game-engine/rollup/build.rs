/*#[cfg(not(clippy))]
fn main() {
    use sp1_helper::build_program_with_args;
    use sp1_helper::BuildArgs; // Add this import
    let args = BuildArgs {
        ignore_rust_version: true,
        docker: false,
        features: vec!["sp1".to_string()],
        ..Default::default()
    };

    build_program_with_args("../zkprogram", args.clone());
    build_program_with_args("../crash_game", args);
}
#[cfg(clippy)]
*/
fn main() {}
