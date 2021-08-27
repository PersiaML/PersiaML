pub use anyhow;
pub use async_channel;
pub use async_compat;
pub use async_executor;
pub use async_lock;
pub use async_oneshot;
pub use bytes;
pub use chrono;
pub use easy_parallel;
pub use flume;
pub use futures;
pub use half;
pub use hashbrown;
pub use indexmap;
pub use itertools;
pub use ndarray;
pub use ndarray_rand;
pub use numpy;
pub use once_cell;
pub use parking_lot;
pub use rand;
pub use rayon;
pub use retry;
pub use serde_bytes;
pub use serde_yaml;
pub use smol;
pub use smol_timeout;
pub use tokio;
pub use tracing;
pub use tracing_subscriber;

#[derive(Clone)]
pub struct ChannelPair<T> {
    pub sender: flume::Sender<T>,
    pub receiver: flume::Receiver<T>,
}

impl<T> ChannelPair<T> {
    pub fn new(cap: usize) -> Self {
        let (sender, receiver) = flume::bounded(cap);
        Self { sender, receiver }
    }

    pub fn new_unbounded() -> Self {
        let (sender, receiver) = flume::unbounded();
        Self { sender, receiver }
    }
}
