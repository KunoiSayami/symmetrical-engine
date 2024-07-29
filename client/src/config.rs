use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    uuid: String,
    remote: Option<String>,
}

impl Config {
    pub async fn read(file: &str) -> anyhow::Result<Self> {
        let mut f = tokio::fs::File::open(file).await?;
        let mut s = String::new();
        f.read_to_string(&mut s).await?;
        Ok(toml::from_str(&s)?)
    }

    pub fn exists(file: &str) -> bool {
        std::path::Path::new(file).exists()
    }

    pub async fn write(&self, file: &str) -> anyhow::Result<()> {
        let mut f = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .open(file)
            .await?;

        f.write_all(toml::to_string_pretty(self).unwrap().as_bytes())
            .await?;

        Ok(())
    }

    pub fn uuid(&self) -> &str {
        &self.uuid
    }

    pub fn remote(&self) -> Option<&str> {
        self.remote.as_ref().map(|x| x.as_str())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            uuid: uuid::Uuid::new_v4().to_string(),
            remote: None,
        }
    }
}
