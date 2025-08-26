use std::path::PathBuf;

use anyhow::Result;
use anyhow::anyhow;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Settings {
    pub output: OutputSettings,
}

impl Settings {
    pub fn new(config_path: Option<PathBuf>) -> Result<Self> {
        use config::Config;

        // We only ensure file exists if the user has not supplied the path.
        let config_path = match config_path {
            Some(path) => path,
            None => Self::ensure_config_path()?,
        };

        let settings = Config::builder()
            .add_source(Config::try_from(&Settings::default())?)
            .add_source(config::File::from(config_path))
            .add_source(config::Environment::with_prefix("LOSRS").separator("__"))
            .build()
            .unwrap();

        let config: Settings = settings.try_deserialize()?;
        Ok(config)
    }

    pub fn ensure_config_path() -> Result<PathBuf> {
        let p = Self::get_config_path()?;

        if !p.exists() {
            confy::store_path(&p, Settings::default())?;
        }

        Ok(p)
    }

    pub fn get_config_path() -> Result<PathBuf> {
        Self::get_configuration_file_path("losrs", "losrs")
    }

    // Inlining code from https://github.com/rust-cli/confy/pull/119 until it is merged.
    fn get_configuration_file_path<'a>(
        app_name: &str,
        config_name: impl Into<Option<&'a str>>,
    ) -> Result<PathBuf> {
        use etcetera::AppStrategy;
        use etcetera::AppStrategyArgs;
        use etcetera::app_strategy::choose_app_strategy;
        let config_name = config_name.into().unwrap_or("default-config");
        let project = choose_app_strategy(AppStrategyArgs {
            top_level_domain: "rs".to_string(),
            author: "".to_string(),
            app_name: app_name.to_string(),
        })
        .map_err(|e| anyhow!(format!("could not determine home directory path: {e}")))?;

        let config_dir_str = project.config_dir().display().to_string();

        let path = [config_dir_str, format!("{config_name}.toml")].iter().collect();

        Ok(path)
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
