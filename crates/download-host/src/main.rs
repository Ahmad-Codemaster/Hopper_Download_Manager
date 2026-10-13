fn main() {
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let root = std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().expect("current directory"));
    let root = root.join("HopperDownloadManager");
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let engine = match download_core::engine::Engine::new(root) {
        Ok(engine) => engine,
        Err(error) => {
            eprintln!("download-host initialization failed: {error}");
            return;
        }
    };
    if let Err(error) = download_host::native_messaging::process_engine_stream(&mut input, &mut output, &engine, &runtime) {
        eprintln!("download-host stopped: {error}");
    }
}
