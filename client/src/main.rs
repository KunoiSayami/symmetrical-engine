mod config;
mod listener;
mod task;
mod web;

use clap::arg;
use config::Config;
use listener::KeyShortcut;
use std::{
    io::Write,
    sync::{atomic::AtomicBool, Arc},
};
use tap::TapFallible;
use tokio::sync::mpsc;
use web::{make_connection, WebEvent};

const REMOTE_ADDRESS: &str = env!("B7_REMOTE");
const TERMINATE_TARGET: &str = env!("B7_TASK_TO_KILL");

async fn load_config(config: String) -> anyhow::Result<Config> {
    Ok(if !Config::exists(&config) {
        let cfg = Config::default();
        cfg.write(&config)
            .await
            .tap_err(|e| log::error!("Write configure file error: {e:?}"))?;
        cfg
    } else {
        Config::read(&config)
            .await
            .tap_err(|e| log::error!("Read configure error: {e:?}"))?
    })
}

async fn async_main(config: String) -> anyhow::Result<()> {
    let cfg = load_config(config).await?;

    let exit_signal = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel(64);
    let uuid = cfg.uuid().to_string();
    let remote = cfg.remote().unwrap_or(REMOTE_ADDRESS).to_string();

    let keyboard_thread = KeyShortcut::start(sender.clone(), exit_signal.clone())?;

    let connection = tokio::spawn(make_connection(remote, uuid, receiver));

    tokio::select! {
            _ = async {
                tokio::signal::ctrl_c().await.ok();
                sender.send(WebEvent::Stop).await.ok();
                exit_signal.store(true, std::sync::atomic::Ordering::Relaxed);
                tokio::signal::ctrl_c().await.ok();
            } => {
    x
            }

            ret = connection => {
                ret??;
            }
        }

    keyboard_thread.wait()?;

    Ok(())
}

fn init_log(systemd: bool) {
    let mut builder = env_logger::Builder::from_default_env();
    builder
        .filter_module("hyper", log::LevelFilter::Warn)
        .filter_module("rustls", log::LevelFilter::Warn);

    if systemd {
        builder.format(|buf, record| writeln!(buf, "[{}] {}", record.level(), record.args()));
    }
    builder.init();
}

fn main() -> anyhow::Result<()> {
    let matches = clap::command!()
        .args(&[
            arg!([CONFIG] "Configure file").default_value("config.toml"),
            arg!(--systemd "Disable time output in log"),
        ])
        .get_matches();

    init_log(matches.get_flag("systemd"));

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async_main(
            matches.get_one::<String>("CONFIG").unwrap().to_string(),
        ))
}
