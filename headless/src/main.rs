fn main() {
    if let Err(error) = llm_wiki_core::engine::run_from_args(None) {
        eprintln!("wiki startup failed: {error}");
        std::process::exit(1);
    }
}
