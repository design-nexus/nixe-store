use nixe_store::{
    model::{Event, Request},
    transaction,
};
use std::io::{BufRead, Read};

fn main() {
    // Fixed tool search paths prevent user-controlled programs from running as root.
    // SAFETY: the helper is single-threaded and has not initialized libraries yet.
    unsafe {
        std::env::set_var("PATH", "/usr/bin:/bin");
        std::env::set_var("LC_ALL", "C.UTF-8");
        libc::umask(0o022);
    }
    let result = (|| -> anyhow::Result<()> {
        anyhow::ensure!(unsafe { libc::geteuid() } == 0, "Run Store operations through Polkit");
        let stdin = std::io::stdin();
        let mut input = stdin.lock();
        let mut line = String::new();
        (&mut input).take(2 * 1024 * 1024).read_line(&mut line)?;
        let request: Request = serde_json::from_str(&line)?;
        transaction::run(request, &mut input)
    })();
    if let Err(error) = result {
        transaction::emit(Event::Error { message: format!("{error:#}") });
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
