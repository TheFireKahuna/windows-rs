fn main() -> windows_window::Result<()> {
    use windows_window::{Window, run};

    let _window = Window::new("This is a sample window").create()?;

    run();

    println!("window closed");
    Ok(())
}
