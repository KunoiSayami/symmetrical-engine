use anyhow::anyhow;
use clap::arg;
use config::Config;
use log::{info, warn};
use monitor::{FileWatchDog, ScanUpdateEventReceiver, ScanUpdateHelper};
use tokio::sync::{broadcast, RwLock};

mod config;
mod monitor;
mod route;
mod types;
use std::{io::Write, sync::Arc};

async fn update_config_thread(
    config: String,
    users: Arc<RwLock<Vec<String>>>,
    mut receiver: ScanUpdateEventReceiver,
) -> anyhow::Result<()> {
    while let Some(event) = receiver.recv().await {
        match event {
            monitor::ScanUpdateEvent::NeedUpdate => {
                let cfg = Config::load(&config)
                    .await
                    .map_err(|e| anyhow!("Load configure error: {e:?}"))?;
                let mut new = cfg.web().clone_users();
                let mut users = users.write().await;
                std::mem::swap(&mut *users, &mut new);
            }
            monitor::ScanUpdateEvent::Exit => break,
        }
    }
    Ok(())
}

async fn async_main(config: String) -> anyhow::Result<()> {
    let cfg = Config::load(&config)
        .await
        .map_err(|e| anyhow!("Load configure error: {e:?}"))?;

    let (sender, _) = broadcast::channel(32);

    let (file_event_sender, file_event_receiver) = ScanUpdateHelper::new(64);

    let users = Arc::new(RwLock::new(cfg.web().clone_users()));

    let watchdog = FileWatchDog::start(config.clone(), file_event_sender.clone());

    let reload_monitor = tokio::spawn(update_config_thread(
        config,
        users.clone(),
        file_event_receiver,
    ));

    let web = tokio::spawn(route::route(cfg.clone(), sender.clone(), users.clone()));

    tokio::select! {
        ret = async {
            tokio::signal::ctrl_c().await?;
            info!("Send exit request");
            sender.send(types::WebBroadcastEvent::ServerQuit).ok();
            file_event_sender.exit().await;
            tokio::signal::ctrl_c().await?;
            warn!("Force exit");
            Ok::<_, anyhow::Error>(())
        } => {ret?;}

        ret = web => {
            ret??;
        }
    }

    watchdog.stop();
    reload_monitor.await??;
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
