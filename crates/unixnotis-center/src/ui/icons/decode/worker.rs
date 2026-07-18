//! Bounded background worker queue for file-backed icons

use std::path::PathBuf;
use std::thread;

use crossbeam_channel as channel;

use super::super::cache::IconKey;
use super::model::{IconSubmitError, IconUpdate};
use super::pipeline::decode_icon_file;

const ICON_DECODE_QUEUE_CAPACITY: usize = 128;

pub(in crate::ui::icons) struct IconWorker {
    pub(super) sender: channel::Sender<IconJob>,
}

pub(super) struct IconJob {
    key: IconKey,
    path: PathBuf,
    size: i32,
    scale: i32,
}

impl IconWorker {
    pub(in crate::ui::icons) fn new(update_tx: async_channel::Sender<IconUpdate>) -> Self {
        // A bounded channel makes overload visible instead of growing memory without limit
        let (sender, receiver) = channel::bounded::<IconJob>(ICON_DECODE_QUEUE_CAPACITY);
        // Two workers keep image parsing responsive without taking over the UI process
        let worker_count = thread::available_parallelism().map_or(1, |count| count.get().min(2));

        for _ in 0..worker_count {
            let receiver = receiver.clone();
            let update_tx = update_tx.clone();
            thread::spawn(move || {
                // Each worker owns one job at a time and returns only bounded raster data
                for job in &receiver {
                    let result = decode_icon_file(&job.path, job.size, job.scale);
                    let _ = update_tx.send_blocking(IconUpdate {
                        key: job.key,
                        result,
                    });
                }
            });
        }
        Self { sender }
    }

    pub(in crate::ui::icons) fn submit_decode(
        &self,
        key: IconKey,
        path: PathBuf,
        size: i32,
        scale: i32,
    ) -> Result<(), IconSubmitError> {
        let job = IconJob {
            key,
            path,
            size,
            scale,
        };
        // Nonblocking submission keeps notification rendering responsive under load
        match self.sender.try_send(job) {
            Ok(()) => Ok(()),
            Err(channel::TrySendError::Full(_)) => Err(IconSubmitError::Full),
            Err(channel::TrySendError::Disconnected(_)) => Err(IconSubmitError::Closed),
        }
    }
}
