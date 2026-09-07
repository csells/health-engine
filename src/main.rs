fn main() {
    if let Err(error) = health_engine::cli::run() {
        eprintln!(r#"{{"error":"{}"}}"#, error.code());
        std::process::exit(1);
    }
}
