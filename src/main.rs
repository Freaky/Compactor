#![cfg_attr(not(test), windows_subsystem = "windows")]
#![cfg_attr(test, windows_subsystem = "console")]
#![allow(non_snake_case)]

mod backend;
mod background;
mod console;
mod compact;
mod compression;
mod config;
mod folder;
mod gui;
mod persistence;

fn setup_panic() {
    std::panic::set_hook(Box::new(|e| {
        if !console::alloc() {
            // No point trying to print without a console...
            return;
        }

        println!(r#"
Oh dear, {app} has crashed.  Sorry :(

You can report this on the website at {website}/issues

Please try to include everything below the line, and give some hints as to what
you were doing - like what folder you were running it on.

#############################################################################

App: {app}, Version: {ver}, Build Date: {date}, Hash: {hash}
"#, app = env!("CARGO_PKG_NAME"), website = env!("CARGO_PKG_HOMEPAGE"),
ver = env!("VERGEN_SEMVER"), date = env!("VERGEN_BUILD_DATE").to_string(), hash = env!("VERGEN_SHA_SHORT"));

        if let Some(s) = e.payload().downcast_ref::<&'static str>() {
             println!("panic: {}", s);
        } else {
             println!("panic: [mysteriously lacks a string representation]");
        }

        println!("\nHit Enter to print the rest of the debug info.");

        let _ = std::io::stdin().read_line(&mut String::new());

        let backtrace = backtrace::Backtrace::new();

        println!("\n{:?}\n", backtrace);
        println!("Hit Enter to continue.");

        let _ = std::io::stdin().read_line(&mut String::new());
    }));
}

fn main() {
    setup_panic();
    console::attach();
    let ret = std::panic::catch_unwind(gui::spawn_gui);
    console::free();

    if ret.is_err() {
        std::process::exit(1);
    }
}
