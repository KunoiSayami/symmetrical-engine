use serde::Deserialize;
use tokio::io::AsyncReadExt;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    web: Web,
}

impl Config {
    pub async fn load(file: &str) -> anyhow::Result<Self> {
        let mut f = tokio::fs::File::open(file).await?;
        let mut s = String::new();

        f.read_to_string(&mut s).await?;
        Ok(toml::from_str(&s)?)
    }

    pub fn web(&self) -> &Web {
        &self.web
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Web {
    bind: String,
    users: Vec<User>,
}

impl Web {
    pub fn bind(&self) -> &str {
        &self.bind
    }

    pub fn clone_users(&self) -> Vec<String> {
        self.users.iter().map(|u| u.uuid().to_string()).collect()
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct User {
    uuid: String,
}

impl User {
    pub fn uuid(&self) -> &str {
        &self.uuid
    }
}

impl Default for Web {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:37001".to_string(),
            users: vec![],
        }
    }
}
