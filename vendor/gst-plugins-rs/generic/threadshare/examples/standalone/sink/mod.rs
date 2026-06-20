pub mod async_mutex;
pub mod sync_mutex;
pub mod task;

mod settings;
pub use settings::Settings;

mod stats;
pub use stats::Stats;

pub const ASYNC_MUTEX_ELEMENT_NAME: &str = "ts-standalone-async-mutex-sink";
pub const SYNC_MUTEX_ELEMENT_NAME: &str = "ts-standalone-sync-mutex-sink";
pub const TASK_ELEMENT_NAME: &str = "ts-standalone-task-sink";

use std::sync::LazyLock;
static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "ts-standalone-sink",
        gst::DebugColorFlags::empty(),
        Some("Thread-sharing standalone test sink"),
    )
});
