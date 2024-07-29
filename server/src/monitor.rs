use std::{path::PathBuf, thread::JoinHandle, time::Duration};

use kstool_helper_generator::Helper;
use log::{debug, error, warn};
use notify::{Event, RecursiveMode, Watcher};
use tap::{TapFallible, TapOptional};
use tokio::sync::oneshot;

#[derive(Clone, Copy, Helper)]
pub enum ScanUpdateEvent {
    NeedUpdate,
    Exit,
}

#[derive(Debug)]
pub struct FileWatchDog {
    handler: JoinHandle<Option<()>>,
    stop_signal_channel: oneshot::Sender<bool>,
}

impl FileWatchDog {
    pub fn file_watching(
        file: String,
        stop_signal_channel: oneshot::Receiver<bool>,
        sender: ScanUpdateHelper,
    ) -> Option<()> {
        let mut watcher = notify::recommended_watcher(move |res| match res {
            Ok(event) => {
                if Self::decide(event) {
                    tokio::runtime::Builder::new_current_thread()
                        .build()
                        .map(|runtime| runtime.block_on(Self::send_event(sender.clone())))
                        .tap_err(|e| error!("[Can be safely ignored] Unable create runtime: {e:?}"))
                        .ok();
                }
            }
            Err(e) => {
                error!("[Can be safely ignored] Got error while watching file {e:?}")
            }
        })
        .tap_err(|e| error!("[Can be safely ignored] Can't start watcher {e:?}"))
        .ok()?;

        let path = PathBuf::from(file);

        watcher
            .watch(&path, RecursiveMode::NonRecursive)
            .tap_err(|e| error!("[Can be safely ignored] Unable to watch file: {e:?}"))
            .ok()?;

        stop_signal_channel
            .blocking_recv()
            .tap_err(|e| {
                error!("[Can be safely ignored] Got error while poll oneshot event: {e:?}")
            })
            .ok();

        watcher
            .unwatch(&path)
            .tap_err(|e| error!("[Can be safely ignored] Unable to unwatch file: {e:?}"))
            .ok()?;

        debug!("File watcher exited!");
        Some(())
    }

    fn decide(event: Event) -> bool {
        if let notify::EventKind::Access(notify::event::AccessKind::Close(
            notify::event::AccessMode::Write,
        )) = event.kind
        {
            return true;
        }
        event.need_rescan()
    }

    async fn send_event(sender: ScanUpdateHelper) -> Option<()> {
        sender.need_update().await.tap_none(|| {
            error!("[Can be safely ignored] Got error while sending event to update thread")
        })
    }

    pub fn start(path: String, sender: ScanUpdateHelper) -> Self {
        let (stop_signal_channel, receiver) = oneshot::channel();
        Self {
            handler: std::thread::spawn(|| Self::file_watching(path, receiver, sender)),
            stop_signal_channel,
        }
    }

    pub fn stop(self) -> Option<()> {
        if !self.handler.is_finished() {
            self.stop_signal_channel
                .send(true)
                .tap_err(|e| {
                    error!(
            "[Can be safely ignored] Unable send terminate signal to file watcher thread: {e:?}",
        )
                })
                .ok()?;
            std::thread::spawn(move || {
                for _ in 0..5 {
                    std::thread::sleep(Duration::from_millis(100));
                    if self.handler.is_finished() {
                        break;
                    }
                }
                if !self.handler.is_finished() {
                    warn!("[Can be safely ignored] File watching not finished yet.");
                }
            })
            .join()
            .unwrap();
        }
        Some(())
    }
}
