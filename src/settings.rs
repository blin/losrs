use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Settings {
    pub output: OutputSettings,
}

impl Settings {
    pub fn new() -> Result<Self> {
        use config::Config;

        let settings = Config::builder()
            .add_source(Config::try_from(&Settings::default())?)
            .add_source(config::Environment::with_prefix("LOSRS").separator("__"))
            .build()
            .unwrap();

        let config: Settings = settings.try_deserialize()?;
        Ok(config)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    Clean,
    Typst,
    Storage,
    Sixel,
    Kitty,
    ITerm,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OutputSettings {
    pub format: OutputFormat,
    pub ppi: f32,
    pub base_font_size: i32,
    pub line_height_scaling: f32,
}

impl Default for OutputSettings {
    fn default() -> Self {
        Self {
            format: OutputFormat::Clean,
            ppi: 96.0,
            base_font_size: 12,
            line_height_scaling: 1.2,
        }
    }
}
