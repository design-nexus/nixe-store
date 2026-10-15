mod prefs;
mod theme;
mod widgets;
mod window;
use gtk::prelude::*;
use gtk::{gio, glib};
fn main() -> glib::ExitCode {
    nixe_core::app::init(nixe_core::app::App { slug: "nixe-store", name: "Store" });
    if std::env::args().any(|a| a == "--help") {
        println!("Usage: nixe-store [--section home|categories|installed|updates|repositories] [--search TEXT]");
        return glib::ExitCode::SUCCESS;
    }
    if std::env::args().any(|a| a == "--blackarch-check") {
        return match nixe_store::blackarch::verify_bootstrap(
            &nixe_core::paths::cache_dir().join("blackarch-verification"),
            |message| println!("{message}"),
        ) {
            Ok(verified) => {
                println!("Verified BlackArch keyring signed by {}", verified.fingerprint);
                glib::ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                glib::ExitCode::FAILURE
            }
        };
    }
    if std::env::args().any(|a| a == "--artwork-check") {
        let result = (|| -> anyhow::Result<()> {
            let cache = nixe_core::paths::cache_dir();
            let catalog = nixe_store::catalog::load(&cache, false)?;
            for name in ["blender", "krita", "obs-studio", "firefox"] {
                let app =
                    catalog.apps.iter().find(|a| a.package == name).ok_or_else(|| anyhow::anyhow!("Missing app: {name}"))?;
                let icon = nixe_store::assets::icon(&cache, &app.icon)?;
                let screenshot = nixe_store::assets::screenshot(&cache, app, 0)?;
                println!("{name}: decoded icon {} and screenshot {}", icon.display(), screenshot.display());
            }
            Ok(())
        })();
        return match result {
            Ok(()) => glib::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                glib::ExitCode::FAILURE
            }
        };
    }
    if std::env::args().any(|a| a == "--catalog-check") {
        return match nixe_store::catalog::load(&nixe_core::paths::cache_dir(), std::env::args().any(|a| a == "--refresh")) {
            Ok(c) => {
                if !c.warning.is_empty() {
                    eprintln!("{}", c.warning);
                    return glib::ExitCode::FAILURE;
                }
                println!(
                    "{} packages; {} desktop entries; {} popular picks",
                    c.apps.len(),
                    c.apps.iter().filter(|a| a.desktop).count(),
                    c.apps.iter().filter(|a| a.popular).count()
                );
                glib::ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                glib::ExitCode::FAILURE
            }
        };
    }
    if std::env::var_os("GSK_RENDERER").is_none() {
        unsafe { std::env::set_var("GSK_RENDERER", "ngl") };
    }
    let app = gtk::Application::builder()
        .application_id("io.github.design_nexus.nixe.Store")
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    app.connect_command_line(|app, cli| {
        let args: Vec<_> = cli.arguments().iter().map(|a| a.to_string_lossy().to_string()).collect();
        let value = |key| args.iter().position(|a| a == key).and_then(|i| args.get(i + 1)).map(String::as_str);
        window::present(app, value("--section"), value("--search"));
        glib::ExitCode::SUCCESS
    });
    app.connect_shutdown(|_| prefs::flush());
    app.run()
}
