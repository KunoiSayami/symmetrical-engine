use std::{
    sync::{atomic::AtomicBool, Arc},
    thread::JoinHandle,
    time::Duration,
};

use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use once_cell::sync::Lazy;
use tap::TapFallible;
use tokio::sync::mpsc;

static HOT_KEY: Lazy<HotKey> = Lazy::new(|| HotKey::new(Some(Modifiers::CONTROL), Code::F6));

use crate::web::WebEvent;

pub struct KeyShortcut {
    handler: JoinHandle<anyhow::Result<()>>,
    manager: GlobalHotKeyManager,
}

impl KeyShortcut {
    pub fn start(
        sender: mpsc::Sender<WebEvent>,
        stop_signal: Arc<AtomicBool>,
    ) -> anyhow::Result<Self> {
        let manager = GlobalHotKeyManager::new().unwrap();
        //let hotkey = HotKey::new(Some(Modifiers::SHIFT), Code::KeyD);
        manager.register(*HOT_KEY)?;

        Ok(Self {
            handler: std::thread::spawn(|| Self::run(sender, stop_signal)),
            manager,
        })
    }
    fn run(sender: mpsc::Sender<WebEvent>, stop_signal: Arc<AtomicBool>) -> anyhow::Result<()> {
        loop {
            while let Ok(_) = GlobalHotKeyEvent::receiver().recv_timeout(Duration::from_secs(1)) {
                sender
                    .blocking_send(WebEvent::SendTerminate)
                    .tap_err(|_| log::error!("Fail to send message to web thread"))
                    .ok();
            }

            if stop_signal.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
        }
        Ok(())
    }

    pub fn wait(self) -> anyhow::Result<()> {
        for _ in 0..3 {
            if !self.handler.is_finished() {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        self.manager
            .unregister(*HOT_KEY)
            .tap_err(|e| log::error!("Error unregister key {e:?}"))?;
        Ok(if self.handler.is_finished() {
            self.handler.join().unwrap()?
        } else {
            ()
        })
    }
}
