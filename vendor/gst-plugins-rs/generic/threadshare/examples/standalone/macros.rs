macro_rules! debug_or_trace {
    ($cat:expr, $raise_log_level:expr, $qual:ident = $obj:expr, $rest:tt $(,)?) => {
        if $raise_log_level {
            gst::debug!($cat, $qual = $obj, $rest);
        } else {
            gst::trace!($cat, $qual = $obj, $rest);
        }
    };
}

macro_rules! log_or_trace {
    ($cat:expr, $raise_log_level:expr, $qual:ident = $obj:expr, $rest:tt $(,)?) => {
        if $raise_log_level {
            gst::log!($cat, $qual = $obj, $rest);
        } else {
            gst::trace!($cat, $qual = $obj, $rest);
        }
    };
}
