fn main() {
    if let Err(error) = desktop_updater::run_helper_from_args() {
        eprintln!("QuotaBarWin updater: {error}");
        std::process::exit(1);
    }
}
