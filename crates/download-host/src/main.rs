fn main() {
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    if let Err(error) = download_host::native_messaging::process_stream(&mut input, &mut output) {
        eprintln!("download-host stopped: {error}");
    }
}
