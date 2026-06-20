use super::CAT;
use clap::Parser;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, clap::ValueEnum)]
pub enum Sink {
    /// Item handling in PadHandler with async Mutex
    AsyncMutex,
    /// Item handling in PadHandler with sync Mutex
    SyncMutex,
    /// Item handling in runtime::Task
    Task,
}

impl Sink {
    pub fn element_name(self) -> &'static str {
        use super::sink;
        use Sink::*;
        match self {
            AsyncMutex => sink::ASYNC_MUTEX_ELEMENT_NAME,
            SyncMutex => sink::SYNC_MUTEX_ELEMENT_NAME,
            Task => sink::TASK_ELEMENT_NAME,
        }
    }
}

#[derive(Parser, Debug)]
#[clap(version)]
#[clap(
    about = "Standalone pipeline threadshare runtime test. Use `GST_DEBUG=ts-standalone*:4` for stats"
)]
pub struct Args {
    /// Parallel streams to process.
    #[clap(short, long, default_value_t = 5000)]
    pub streams: u32,

    /// Threadshare groups.
    #[clap(short, long, default_value_t = 2)]
    pub groups: u32,

    /// Threadshare Context wait in ms (max throttling duration).
    #[clap(short, long, default_value_t = 20)]
    pub wait: u32,

    /// Buffer push period in ms.
    #[clap(short, long, default_value_t = 20)]
    pub push_period: u32,

    /// Number of buffers per stream to output before sending EOS (-1 = unlimited).
    #[clap(short, long, default_value_t = 5000)]
    pub num_buffers: i32,

    /// The Sink variant to use.
    #[clap(long, value_enum, default_value_t = Sink::Task)]
    pub sink: Sink,

    /// Disables statistics logging.
    #[clap(short, long)]
    pub disable_stats_log: bool,
}

pub fn args() -> Args {
    let args = Args::parse();
    gst::info!(CAT, "{:?}", args);

    args
}
