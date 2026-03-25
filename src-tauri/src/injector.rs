use enigo::{Enigo, Keyboard, Settings};
use std::error::Error;

pub struct Injector {
    enigo: Enigo,
}

impl Injector {
    pub fn new() -> Result<Self, Box<dyn Error + Send + Sync>> {
        let enigo = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
        Ok(Self { enigo })
    }

    pub fn type_text(&mut self, text: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.enigo.text(text).map_err(|e| e.to_string())?;
        Ok(())
    }
}
