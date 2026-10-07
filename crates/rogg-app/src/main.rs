fn main() {
    match rogg_app::args::Args::parse(std::env::args_os().skip(1)) {
        Ok(mut args) => {
            // Data piped or redirected into rogg opens as a live tab (spec §4.1).
            if rogg_core::stream::stdin_has_data() && !args.paths.iter().any(|p| p.as_os_str() == "-") {
                args.paths.push("-".into());
            }
            rogg_app::run(args)
        }
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(if message == rogg_app::args::USAGE { 0 } else { 2 });
        }
    }
}
