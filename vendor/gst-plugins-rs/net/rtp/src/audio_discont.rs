// SPDX-License-Identifier: MPL-2.0

use gst::{
    glib::{self, prelude::*},
    prelude::*,
};

#[derive(Debug, Default)]
pub struct AudioDiscont {
    /// If last processing detected a discontinuity.
    discont_pending: bool,
    /// Base PTS to which the offsets below are relative.
    base_pts: Option<gst::ClockTime>,
    /// Next output sample offset, i.e. offset of the first sample of the queued buffers.
    ///
    /// This is only set once the packet with the base PTS is output.
    next_out_offset: Option<u64>,
    /// Next expected input sample offset.
    next_in_offset: Option<u64>,
    /// PTS of the last buffer that was above the alignment threshold.
    ///
    /// This is reset whenever the next buffer is actually below the alignment threshold again.
    /// FIXME: Should this be running time?
    discont_time: Option<gst::ClockTime>,
    /// Last known sample rate.
    last_rate: Option<u32>,
}

impl AudioDiscont {
    pub fn process_input(
        &mut self,
        settings: &AudioDiscontConfiguration,
        discont: bool,
        rate: u32,
        pts: gst::ClockTime,
        num_samples: usize,
    ) -> bool {
        if self.discont_pending {
            return true;
        }

        if self.last_rate.is_some_and(|last_rate| last_rate != rate) {
            self.discont_pending = true;
        }
        self.last_rate = Some(rate);

        if discont {
            self.discont_pending = true;
            return true;
        }

        // If we have no base PTS yet, this is the first buffer and there's a discont
        let Some(base_pts) = self.base_pts else {
            self.discont_pending = true;
            return true;
        };

        // Never detect a discont if alignment threshold is not set
        let Some(alignment_threshold) = settings.alignment_threshold else {
            return false;
        };

        let expected_pts = base_pts
            + gst::ClockTime::from_nseconds(
                self.next_in_offset
                    .unwrap_or(0)
                    .mul_div_ceil(gst::ClockTime::SECOND.nseconds(), rate as u64)
                    .unwrap(),
            );

        let mut discont = false;

        let diff = pts.into_positive() - expected_pts.into_positive();
        if diff.abs() >= alignment_threshold {
            let mut resync = false;
            if settings.discont_wait.is_zero() {
                resync = true;
            } else if let Some(discont_time) = self.discont_time {
                if (discont_time.into_positive() - pts.into_positive()).abs()
                    >= settings.discont_wait.into_positive()
                {
                    resync = true;
                }
            } else if (expected_pts.into_positive() - pts.into_positive()).abs()
                >= settings.discont_wait.into_positive()
            {
                resync = true;
            } else {
                self.discont_time = Some(expected_pts);
            }

            if resync {
                discont = true;
            }
        } else {
            self.discont_time = None;
        }

        self.next_in_offset = Some(self.next_in_offset.unwrap_or(0) + num_samples as u64);

        if discont {
            self.discont_pending = true;
        }

        discont
    }

    pub fn resync(&mut self, base_pts: gst::ClockTime, num_samples: usize) {
        self.discont_pending = false;
        self.base_pts = Some(base_pts);
        self.next_in_offset = Some(num_samples as u64);
        self.next_out_offset = None;
        self.discont_time = None;
        self.last_rate = None;
    }

    pub fn reset(&mut self) {
        *self = AudioDiscont::default();
    }

    pub fn base_pts(&self) -> Option<gst::ClockTime> {
        self.base_pts
    }

    pub fn next_output_offset(&self) -> Option<u64> {
        self.next_out_offset
    }

    pub fn process_output(&mut self, num_samples: usize) {
        self.next_out_offset = Some(self.next_out_offset.unwrap_or(0) + num_samples as u64);
    }
}

#[derive(Debug, Copy, Clone)]
pub struct AudioDiscontConfiguration {
    pub alignment_threshold: Option<gst::ClockTime>,
    pub discont_wait: gst::ClockTime,
}

impl Default for AudioDiscontConfiguration {
    fn default() -> Self {
        AudioDiscontConfiguration {
            alignment_threshold: Some(gst::ClockTime::from_mseconds(40)),
            discont_wait: gst::ClockTime::from_seconds(1),
        }
    }
}

impl AudioDiscontConfiguration {
    pub fn create_pspecs() -> Vec<glib::ParamSpec> {
        vec![
            glib::ParamSpecUInt64::builder("alignment-threshold")
                .nick("Alignment Threshold")
                .blurb("Timestamp alignment threshold in nanoseconds")
                .default_value(
                    Self::default()
                        .alignment_threshold
                        .map(gst::ClockTime::nseconds)
                        .unwrap_or(u64::MAX),
                )
                .mutable_playing()
                .build(),
            glib::ParamSpecUInt64::builder("discont-wait")
                .nick("Discont Wait")
                .blurb("Window of time in nanoseconds to wait before creating a discontinuity")
                .default_value(Self::default().discont_wait.nseconds())
                .mutable_playing()
                .build(),
        ]
    }

    pub fn set_property(&mut self, value: &glib::Value, pspec: &glib::ParamSpec) -> bool {
        match pspec.name() {
            "alignment-threshold" => {
                self.alignment_threshold = value.get().unwrap();
                true
            }
            "discont-wait" => {
                self.discont_wait = value.get().unwrap();
                true
            }
            _ => false,
        }
    }

    pub fn property(&self, pspec: &glib::ParamSpec) -> Option<glib::Value> {
        match pspec.name() {
            "alignment-threshold" => Some(self.alignment_threshold.to_value()),
            "discont-wait" => Some(self.discont_wait.to_value()),
            _ => None,
        }
    }
}
