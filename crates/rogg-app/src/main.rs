fn main() {
    match rogg_app::args::Args::parse(std::env::args_os().skip(1)) {
        Ok(args) => rogg_app::run(args),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(if message == rogg_app::args::USAGE { 0 } else { 2 });
        }
    }
}
