// Release modunda Windows'ta ek konsol penceresi açılmasın.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use kaans_terminal_lib::host;

fn main() {
    // Aynı ikili dosya oturum sunucusu olarak da çalışır: `kaans-terminal --session-host <soket>`.
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    // Claude Code statusLine komutu: rate_limits'i yerel dosyaya yazar, kullanıcının önceki statusLine'ını çalıştırır.
    if args
        .get(1)
        .is_some_and(|a| a == kaans_terminal_lib::integrations::STATUSLINE_ARG)
    {
        let code = kaans_terminal_lib::integrations::app_data_dir()
            .map_or(0, |d| kaans_terminal_lib::integrations::run_statusline(&d));
        std::process::exit(code);
    }
    if args.get(1).is_some_and(|a| a == host::HOST_ARG) {
        let socket = args
            .get(2)
            .map(std::path::PathBuf::from)
            .unwrap_or_else(host::default_socket);
        if let Err(e) = host::server::run(&socket) {
            eprintln!("[host] {e}");
            std::process::exit(1);
        }
        return;
    }
    kaans_terminal_lib::run()
}
