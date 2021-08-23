use vergen::{vergen, Config};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    vergen(Config::default())?;
    Ok(())
}
