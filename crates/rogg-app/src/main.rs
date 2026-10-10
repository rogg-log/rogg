// A Windows release build is a GUI-subsystem program, so starting it opens no console window. Piped stdin still
// arrives (`type file.log | rogg.exe -`); debug builds keep the console for their output.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    match rogg_app::args::Args::parse(std::env::args_os().skip(1)) {
        Ok(mut args) => {
            if args.version {
                println!("{}", rogg_app::about::version_line(&rogg_app::about::build_info()));
                return;
            }
            // Data piped or redirected into rogg opens as a live tab (spec §4.1).
            if rogg_core::platform::stdin_has_data() && !args.paths.iter().any(|p| p.as_os_str() == "-") {
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
