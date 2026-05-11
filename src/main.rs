use sts2_mod_manager::cli;

fn main() {
    if let Err(error) = cli::run_from_env() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
