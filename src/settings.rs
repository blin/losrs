use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Config {
    pub output: OutputSettings,
}

impl Config {
    pub fn new() -> Result<Self> {
        use figment::Figment;
        use figment::providers::Env;
        use figment::providers::Serialized;
        let config: Config = Figment::new()
            .merge(Serialized::defaults(Config::default()))
            .merge(Env::prefixed("LOSRS__").split("__"))
            .extract()?;
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
