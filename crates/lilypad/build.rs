// Windows version resource. SignPath signs only an .exe whose ProductName is
// the project name and whose ProductVersion is set, and Windows shows
// FileDescription as the program's name in Task Manager and file dialogs.
//
// Decided on the target, not with cfg!(windows): a build script runs on the
// host, so cfg! would describe the machine building Lilypad, not the one it
// is built for.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // FileVersion and ProductVersion default to CARGO_PKG_VERSION, the
        // workspace version release-please bumps.
        winresource::WindowsResource::new()
            .set("ProductName", "Lilypad")
            .set("FileDescription", "Lilypad")
            .compile()
            .expect("failed to compile the Windows version resource");
    }
}
