use gst::glib;
use gst::prelude::*;

use std::time::Duration;

const DEFAULT_CONTEXT: &str = "";
const DEFAULT_CONTEXT_WAIT: Duration = Duration::from_millis(20);
const DEFAULT_PUSH_PERIOD: Duration = Duration::from_millis(20);

#[derive(Debug, Clone)]
pub struct Settings {
    pub context: String,
    pub context_wait: Duration,
    pub is_main_elem: bool,
    pub logs_stats: bool,
    pub push_period: Duration,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            context: DEFAULT_CONTEXT.into(),
            context_wait: DEFAULT_CONTEXT_WAIT,
            is_main_elem: false,
            logs_stats: false,
            push_period: DEFAULT_PUSH_PERIOD,
        }
    }
}

impl Settings {
    pub fn properties() -> Vec<glib::ParamSpec> {
        vec![
            glib::ParamSpecString::builder("context")
                .nick("Context")
                .blurb("Context name to share threads with")
                .default_value(Some(DEFAULT_CONTEXT))
                .build(),
            glib::ParamSpecUInt::builder("context-wait")
                .nick("Context Wait")
                .blurb("Throttle poll loop to run at most once every this many ms")
                .maximum(1000)
                .default_value(DEFAULT_CONTEXT_WAIT.as_millis() as u32)
                .build(),
            glib::ParamSpecBoolean::builder("main-elem")
                .nick("Main Element")
                .blurb("Declare this element as the main one")
                .write_only()
                .build(),
            glib::ParamSpecBoolean::builder("logs-stats")
                .nick("Logs Stats")
                .blurb("Whether statistics should be logged")
                .write_only()
                .build(),
            glib::ParamSpecUInt::builder("push-period")
                .nick("Src buffer Push Period")
                .blurb("Push period used by `src` element (used for stats warnings)")
                .default_value(DEFAULT_PUSH_PERIOD.as_millis() as u32)
                .build(),
        ]
    }

    pub fn set_property(&mut self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "context" => {
                self.context = value
                    .get::<Option<String>>()
                    .unwrap()
                    .unwrap_or_else(|| DEFAULT_CONTEXT.into());
            }
            "context-wait" => {
                self.context_wait = Duration::from_millis(value.get::<u32>().unwrap().into());
            }
            "main-elem" => {
                self.is_main_elem = value.get::<bool>().unwrap();
            }
            "logs-stats" => {
                let logs_stats = value.get().unwrap();
                self.logs_stats = logs_stats;
            }
            "push-period" => {
                self.push_period = Duration::from_millis(value.get::<u32>().unwrap().into());
            }
            _ => unimplemented!(),
        }
    }

    pub fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "context" => self.context.to_value(),
            "context-wait" => (self.context_wait.as_millis() as u32).to_value(),
            "main-elem" => self.is_main_elem.to_value(),
            "push-period" => (self.push_period.as_millis() as u32).to_value(),
            _ => unimplemented!(),
        }
    }
}
