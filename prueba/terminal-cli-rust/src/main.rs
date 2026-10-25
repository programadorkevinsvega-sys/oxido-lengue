use terminal_cli::application::Terminal;
use terminal_cli::infrastructure::Console;

fn main() -> std::io::Result<()> {
    Terminal::new(Console).run()
}
