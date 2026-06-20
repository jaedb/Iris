// Copyright (C) 2020 Mathieu Duponchelle <mathieu@centricular.com>
// Copyright (C) 2021 Jan Schmidt <jan@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst::{debug, log, trace};

use std::sync::LazyLock;

use parking_lot::{Condvar, Mutex, MutexGuard};
use std::sync::atomic::{AtomicU32, Ordering};

const PROP_PRIORITY: &str = "priority";
const PROP_IS_HEALTHY: &str = "is-healthy";

const PROP_ACTIVE_PAD: &str = "active-pad";
const PROP_AUTO_SWITCH: &str = "auto-switch";
const PROP_IMMEDIATE_FALLBACK: &str = "immediate-fallback";
const PROP_LATENCY: &str = "latency";
const PROP_MIN_UPSTREAM_LATENCY: &str = "min-upstream-latency";
const PROP_TIMEOUT: &str = "timeout";
const PROP_STOP_ON_EOS: &str = "stop-on-eos";

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "fallbackswitch",
        gst::DebugColorFlags::empty(),
        Some("Automatic priority-based input selector"),
    )
});

/* Mutex locking ordering:
    - self.settings
    - self.state
    - self.active_sinkpad
    - pad.settings
    - pad.state
*/

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum CapsInfo {
    None,
    Audio(gst_audio::AudioInfo),
    Video(gst_video::VideoInfo),
}

#[derive(Clone, Debug)]
struct Settings {
    timeout: gst::ClockTime,
    latency: gst::ClockTime,
    min_upstream_latency: gst::ClockTime,
    immediate_fallback: bool,
    auto_switch: bool,
    stop_on_eos: bool,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            timeout: gst::ClockTime::SECOND,
            latency: gst::ClockTime::ZERO,
            min_upstream_latency: gst::ClockTime::ZERO,
            immediate_fallback: false,
            auto_switch: true,
            stop_on_eos: false,
        }
    }
}

#[derive(Debug)]
struct State {
    upstream_latency: gst::ClockTime,
    timed_out: bool,
    switched_pad: bool,
    discont_pending: bool,
    first: bool,

    output_running_time: Option<gst::ClockTime>,

    timeout_running_time: Option<gst::ClockTime>,
    timeout_clock_id: Option<gst::ClockId>,

    /// If the src pad is currently busy. Should be checked and waited on using `src_busy_cond`
    /// before calling anything requiring the stream lock.
    src_busy: bool,
}

impl Default for State {
    fn default() -> State {
        State {
            upstream_latency: gst::ClockTime::ZERO,
            timed_out: false,
            switched_pad: false,
            discont_pending: true,
            first: true,

            output_running_time: None,

            timeout_running_time: None,
            timeout_clock_id: None,

            src_busy: false,
        }
    }
}

impl State {
    fn cancel_timeout(&mut self) {
        /* clear any previous timeout */
        if let Some(clock_id) = self.timeout_clock_id.take() {
            clock_id.unschedule();
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.cancel_timeout();
    }
}

#[derive(Debug)]
pub struct FallbackSwitchSinkPad {
    state: Mutex<SinkState>,
    settings: Mutex<SinkSettings>,
}

#[glib::object_subclass]
impl ObjectSubclass for FallbackSwitchSinkPad {
    const NAME: &'static str = "GstFallbackSwitchSinkPad";
    type Type = super::FallbackSwitchSinkPad;
    type ParentType = gst::Pad;

    fn new() -> Self {
        Self {
            state: Mutex::new(SinkState::default()),
            settings: Mutex::new(SinkSettings::default()),
        }
    }
}

impl GstObjectImpl for FallbackSwitchSinkPad {}

impl ObjectImpl for FallbackSwitchSinkPad {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt::builder(PROP_PRIORITY)
                    .nick("Stream Priority")
                    .blurb(
                        "Selection priority for this stream (lower number has a higher priority)",
                    )
                    .default_value(SinkSettings::default().priority)
                    .build(),
                glib::ParamSpecBoolean::builder(PROP_IS_HEALTHY)
                    .nick("Stream Health")
                    .blurb("Whether this stream is healthy")
                    .default_value(false)
                    .read_only()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            PROP_PRIORITY => {
                let mut settings = self.settings.lock();
                let priority = value.get().expect("type checked upstream");
                settings.priority = priority;
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            PROP_PRIORITY => {
                let settings = self.settings.lock();
                settings.priority.to_value()
            }
            PROP_IS_HEALTHY => {
                let state = self.state.lock();
                state.is_healthy.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

impl PadImpl for FallbackSwitchSinkPad {}

impl FallbackSwitchSinkPad {}

#[derive(Clone, Debug, Default)]
struct SinkSettings {
    priority: u32,
}

#[derive(Debug)]
struct SinkState {
    is_healthy: bool,

    segment: gst::FormattedSegment<gst::ClockTime>,
    caps_info: CapsInfo,

    current_running_time: Option<gst::ClockTime>,
    flushing: bool,
    clock_id: Option<gst::SingleShotClockId>,
    /// true if the sink pad has received eos
    eos: bool,
}

impl Default for SinkState {
    fn default() -> Self {
        Self {
            is_healthy: false,

            segment: gst::FormattedSegment::new(),
            caps_info: CapsInfo::None,

            current_running_time: gst::ClockTime::NONE,
            flushing: false,
            clock_id: None,
            eos: false,
        }
    }
}

impl SinkState {
    fn flush_start(&mut self) {
        self.flushing = true;
        if let Some(clock_id) = self.clock_id.take() {
            clock_id.unschedule();
        }
    }
    fn cancel_wait(&mut self) {
        if let Some(clock_id) = self.clock_id.take() {
            clock_id.unschedule();
        }
    }
    fn reset(&mut self) {
        self.flushing = false;
        self.caps_info = CapsInfo::None;
        self.eos = false;
    }

    fn clip_buffer(&self, mut buffer: gst::Buffer) -> Option<gst::Buffer> {
        match &self.caps_info {
            CapsInfo::Audio(audio_info) => gst_audio::audio_buffer_clip(
                buffer,
                self.segment.upcast_ref(),
                audio_info.rate(),
                audio_info.bpf(),
            ),
            CapsInfo::Video(video_info) => {
                let start_ts = buffer.pts();
                let duration = buffer.duration().or_else(|| {
                    if video_info.fps().numer() > 0 {
                        gst::ClockTime::SECOND.mul_div_floor(
                            video_info.fps().denom() as u64,
                            video_info.fps().numer() as u64,
                        )
                    } else {
                        None
                    }
                });
                let end_ts = start_ts.opt_saturating_add(duration);
                let (clipped_start_ts, clipped_end_ts) = self.segment.clip(start_ts, end_ts)?;

                let clipped_duration = clipped_end_ts.opt_sub(clipped_start_ts);
                if clipped_start_ts != start_ts || clipped_duration != buffer.duration() {
                    let buffer = buffer.make_mut();
                    buffer.set_pts(clipped_start_ts);
                    buffer.set_duration(clipped_duration);
                }

                Some(buffer)
            }
            CapsInfo::None => {
                let start_ts = buffer.pts();
                let end_ts = start_ts.opt_saturating_add(buffer.duration());

                // Can only clip buffers completely away, i.e. drop them, if they're raw
                if let Some((clipped_start_ts, clipped_end_ts)) =
                    self.segment.clip(start_ts, end_ts)
                {
                    let clipped_duration = clipped_end_ts.opt_sub(clipped_start_ts);
                    if clipped_start_ts != start_ts || clipped_duration != buffer.duration() {
                        let buffer = buffer.make_mut();
                        buffer.set_pts(clipped_start_ts);
                        buffer.set_duration(clipped_duration);
                    }
                }

                Some(buffer)
            }
        }
    }

    fn get_sync_time(
        &self,
        buffer: &gst::Buffer,
    ) -> (Option<gst::ClockTime>, Option<gst::ClockTime>) {
        let last_ts = self.current_running_time;
        let duration = buffer.duration().unwrap_or(gst::ClockTime::ZERO);

        let start_ts = match buffer.dts_or_pts() {
            Some(ts) => ts,
            None => return (last_ts, last_ts),
        };
        let end_ts = start_ts.saturating_add(duration);

        match self.segment.clip(start_ts, end_ts) {
            Some((start_ts, end_ts)) => (
                self.segment.to_running_time(start_ts),
                self.segment.to_running_time(end_ts),
            ),
            None => (None, None),
        }
    }

    fn schedule_clock(
        &mut self,
        imp: &FallbackSwitch,
        pad: &super::FallbackSwitchSinkPad,
        running_time: Option<gst::ClockTime>,
        extra_time: gst::ClockTime,
    ) -> Option<gst::SingleShotClockId> {
        let running_time = running_time?;
        let clock = imp.obj().clock()?;
        let base_time = imp.obj().base_time()?;

        let wait_until = running_time + base_time;
        let wait_until = wait_until.saturating_add(extra_time);

        let now = clock.time();

        /* If the buffer is already late, skip the clock wait */
        if wait_until < now {
            debug!(
                CAT,
                obj = pad,
                "Skipping buffer wait until {} - clock already {}",
                wait_until,
                now
            );
            return None;
        }

        debug!(
            CAT,
            obj = pad,
            "Scheduling buffer wait until {} = {} + extra {} + base time {}",
            wait_until,
            running_time,
            extra_time,
            base_time
        );

        let clock_id = clock.new_single_shot_id(wait_until);
        self.clock_id = Some(clock_id.clone());
        Some(clock_id)
    }

    fn is_healthy(
        &self,
        pad: &super::FallbackSwitchSinkPad,
        state: &State,
        settings: &Settings,
        now_running_time: Option<gst::ClockTime>,
    ) -> bool {
        /* The pad is healthy if it has received data within the
         * last 'timeout' duration, which means the pad's current_running_time+timeout
         * is later than 'now' according to the passed in running time, but not later
         * than the timeout_running_time that would mean we time out before outputting
         * that buffer */
        match (
            self.current_running_time,
            now_running_time,
            state.timeout_running_time,
        ) {
            (Some(pad_running_time), Some(now_running_time), Some(global_timeout_running_time)) => {
                let timeout_running_time = pad_running_time.saturating_add(settings.timeout);
                log!(
                    CAT,
                    obj = pad,
                    "pad_running_time {} timeout_running_time {} now_running_time {}",
                    pad_running_time,
                    timeout_running_time,
                    now_running_time,
                );

                timeout_running_time > now_running_time // Must be > not >=
                    && pad_running_time <= global_timeout_running_time
            }
            (Some(pad_running_time), Some(now_running_time), None) => {
                let timeout_running_time = pad_running_time.saturating_add(settings.timeout);
                log!(
                    CAT,
                    obj = pad,
                    "pad_running_time {} timeout_running_time {} now_running_time {}",
                    pad_running_time,
                    timeout_running_time,
                    now_running_time,
                );

                timeout_running_time > now_running_time // Must be > not >=
            }
            (Some(_input_running_time), None, _) => true,
            (None, _, _) => false,
        }
    }
}

#[derive(Debug)]
pub struct FallbackSwitch {
    state: Mutex<State>,
    src_busy_cond: Condvar,
    settings: Mutex<Settings>,

    // Separated from the rest of the `state` because it can be
    // read from a property notify, which is prone to deadlocks.
    // https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/issues/200
    active_sinkpad: Mutex<Option<super::FallbackSwitchSinkPad>>,

    src_pad: gst::Pad,
    sink_pad_serial: AtomicU32,
}

impl GstObjectImpl for FallbackSwitch {}

impl FallbackSwitch {
    fn set_active_pad(&self, state: &mut State, pad: &super::FallbackSwitchSinkPad) {
        let prev_active_pad = self.active_sinkpad.lock().replace(pad.clone());
        if prev_active_pad.as_ref() == Some(pad) {
            return;
        }

        state.switched_pad = true;
        state.discont_pending = true;

        let mut pad_state = pad.imp().state.lock();
        pad_state.cancel_wait();
        drop(pad_state);

        debug!(CAT, obj = pad, "Now active pad");
    }

    fn handle_timeout(&self, state: &mut State, settings: &Settings) {
        debug!(
            CAT,
            imp = self,
            "timeout fired - looking for a pad to switch to"
        );

        /* Advance the output running time to this timeout */
        state.output_running_time = state.timeout_running_time;

        if !settings.auto_switch {
            /* If auto-switching is disabled, don't check for a new
             * pad */
            state.timed_out = true;
            return;
        }

        let active_sinkpad = self.active_sinkpad.lock().clone();

        let mut best_priority = 0u32;
        let mut best_pad = None;

        let now_running_time = state.timeout_running_time;

        for pad in self.obj().sink_pads() {
            /* Don't consider the active sinkpad */
            let pad = pad.downcast_ref::<super::FallbackSwitchSinkPad>().unwrap();
            let pad_imp = pad.imp();
            if active_sinkpad.as_ref() == Some(pad) {
                continue;
            }
            let pad_settings = pad_imp.settings.lock().clone();
            let pad_state = pad_imp.state.lock();
            #[allow(clippy::collapsible_if)]
            /* If this pad has data that arrived within the 'timeout' window
             * before the timeout fired, we can switch to it */
            if pad_state.is_healthy(pad, state, settings, now_running_time) {
                if best_pad.is_none() || pad_settings.priority < best_priority {
                    best_pad = Some(pad.clone());
                    best_priority = pad_settings.priority;
                }
            }
        }

        if let Some(best_pad) = best_pad {
            debug!(
                CAT,
                imp = self,
                "Found viable pad to switch to: {:?}",
                best_pad
            );
            self.set_active_pad(state, &best_pad)
        } else {
            state.timed_out = true;
        }
    }

    fn on_timeout(&self, clock_id: &gst::ClockId) {
        let settings = self.settings.lock().clone();
        let mut state = self.state.lock();

        if state.timeout_clock_id.as_ref() != Some(clock_id) {
            /* Timeout fired late, ignore it. */
            debug!(CAT, imp = self, "Late timeout callback. Ignoring");
            return;
        }

        // Ensure sink_chain on an inactive pad can schedule another timeout
        state.timeout_clock_id = None;

        self.handle_timeout(&mut state, &settings);
        let changed = self.update_health_statuses(&state, &settings);
        drop(state);
        for pad in changed {
            pad.notify(PROP_IS_HEALTHY);
        }
    }

    fn cancel_waits(&self) {
        for pad in self.obj().sink_pads() {
            let pad = pad.downcast_ref::<super::FallbackSwitchSinkPad>().unwrap();
            let pad_imp = pad.imp();
            let mut pad_state = pad_imp.state.lock();
            pad_state.cancel_wait();
        }
    }

    fn schedule_timeout(
        &self,
        state: &mut State,
        settings: &Settings,
        running_time: gst::ClockTime,
    ) -> bool {
        state.cancel_timeout();

        let clock = match self.obj().clock() {
            None => return false,
            Some(clock) => clock,
        };

        let base_time = match self.obj().base_time() {
            Some(base_time) => base_time,
            None => return false,
        };

        let timeout_running_time = running_time
            .saturating_add(state.upstream_latency + settings.timeout + settings.latency);
        let wait_until = timeout_running_time + base_time;
        state.timeout_running_time = Some(timeout_running_time);

        /* If we're already running behind, fire the timeout immediately */
        let now = clock.time();
        if wait_until <= now {
            self.handle_timeout(state, settings);
            return true;
        }

        debug!(CAT, imp = self, "Scheduling timeout for {}", wait_until);
        let timeout_id = clock.new_single_shot_id(wait_until);

        state.timeout_clock_id = Some(timeout_id.clone().into());
        state.timed_out = false;

        let imp_weak = self.downgrade();
        timeout_id
            .wait_async(move |_clock, _time, clock_id| {
                let Some(imp) = imp_weak.upgrade() else {
                    return;
                };
                imp.on_timeout(clock_id);
            })
            .expect("Failed to wait async");
        false
    }

    fn update_health_statuses(
        &self,
        state: &State,
        settings: &Settings,
    ) -> Vec<super::FallbackSwitchSinkPad> {
        let mut changed = Vec::<super::FallbackSwitchSinkPad>::new();

        /* Iterate over sink pads and update their is_healthy status,
         * returning a Vec of pads whose health changed and need notifying */
        for pad in self.obj().sink_pads() {
            let pad = pad.downcast_ref::<super::FallbackSwitchSinkPad>().unwrap();
            let pad_imp = pad.imp();
            let mut pad_state = pad_imp.state.lock();

            /* If this pad has data that arrived within the 'timeout' window
             * before the timeout fired, we can switch to it */
            let is_healthy = pad_state.is_healthy(pad, state, settings, state.output_running_time);
            let health_changed = is_healthy != pad_state.is_healthy;
            pad_state.is_healthy = is_healthy;

            drop(pad_state);

            if health_changed {
                log!(CAT, obj = pad, "Health changed to {}", is_healthy);
                changed.push(pad.clone());
            }
        }

        changed
    }

    fn sink_activatemode(
        pad: &super::FallbackSwitchSinkPad,
        _mode: gst::PadMode,
        activate: bool,
    ) -> Result<(), gst::LoggableError> {
        let mut pad_state = pad.imp().state.lock();
        if activate {
            pad_state.reset();
        } else {
            pad_state.flush_start();
        }

        Ok(())
    }

    fn sink_chain(
        &self,
        pad: &super::FallbackSwitchSinkPad,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        self.chain(pad, buffer, None)
    }

    fn chain(
        &self,
        pad: &super::FallbackSwitchSinkPad,
        buffer: gst::Buffer,
        from_gap: Option<&gst::event::Gap>,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        let settings = self.settings.lock().clone();
        let mut state = self.state.lock();
        let pad = pad.downcast_ref::<super::FallbackSwitchSinkPad>().unwrap();
        let pad_imp = pad.imp();

        if settings.stop_on_eos && self.has_sink_pad_eos() {
            debug!(CAT, obj = pad, "return eos as stop-on-eos is enabled");
            return Err(gst::FlowError::Eos);
        }

        let mut buffer = {
            let pad_state = pad_imp.state.lock();
            trace!(
                CAT,
                obj = pad,
                "Clipping {:?} against segment {:?}",
                buffer,
                pad_state.segment,
            );
            match pad_state.clip_buffer(buffer) {
                Some(buffer) => buffer,
                None => {
                    log!(
                        CAT,
                        obj = pad,
                        "Dropping raw buffer completely out of segment",
                    );

                    return Ok(gst::FlowSuccess::Ok);
                }
            }
        };

        /* There are 4 cases coming in:
         *  1. This is not the active pad but is higher priority:
         *    - become the active pad, then goto 4.
         *  2. This is not the active pad, but the output timed out due to all pads running
         *     late.
         *    - become the active pad, then goto 4.
         *  3. This is not the active pad, but might become the active pad
         *    - Wait for the buffer end time (or buffer start time + timeout if there's no
         *    duration). If we get woken early, and became the active pad, then output the
         *    buffer.
         *  4. This is the active pad:
         *    - sleep until the buffer running time, then check if we're still active
         */

        /* see if we should become the active pad */
        let active_sinkpad = self.active_sinkpad.lock().clone();
        let mut is_active = active_sinkpad.as_ref() == Some(pad);
        if !is_active && settings.auto_switch {
            let pad_settings = pad_imp.settings.lock().clone();
            let mut switch_to_pad = state.timed_out;

            switch_to_pad |= if let Some(active_sinkpad) = &active_sinkpad {
                let active_sinkpad_imp = active_sinkpad.imp();
                let active_pad_settings = active_sinkpad_imp.settings.lock().clone();
                (pad_settings.priority < active_pad_settings.priority)
                    || (state.first && settings.immediate_fallback)
            } else {
                match settings.immediate_fallback {
                    true => true,
                    false => pad_settings.priority == 0,
                }
            };
            if state.first {
                state.first = false;
            }

            if switch_to_pad {
                state.timed_out = false;
                self.set_active_pad(&mut state, pad);
                is_active = true;
            }
        }

        let mut pad_state = pad_imp.state.lock();
        let raw_pad = !matches!(pad_state.caps_info, CapsInfo::None);
        let (start_running_time, end_running_time) = pad_state.get_sync_time(&buffer);

        if let Some(running_time) = start_running_time {
            pad_state.current_running_time = Some(running_time);
        }

        /* Update pad is-healthy state if necessary and notify
         * if it changes, as that might affect which pad is
         * active */
        let is_healthy = pad_state.is_healthy(pad, &state, &settings, state.output_running_time);
        let health_changed = is_healthy != pad_state.is_healthy;
        pad_state.is_healthy = is_healthy;

        /* Need to drop state locks before notifying */
        let (mut state, mut pad_state) = if health_changed {
            drop(pad_state);
            drop(state);
            log!(CAT, obj = pad, "Health changed to {}", is_healthy);
            pad.notify(PROP_IS_HEALTHY);

            if !settings.auto_switch {
                /* Re-check if this is the active sinkpad */
                let active_sinkpad = self.active_sinkpad.lock().clone();
                is_active = active_sinkpad.as_ref() == Some(pad);
            }

            (self.state.lock(), pad_imp.state.lock())
        } else {
            (state, pad_state)
        };

        log!(
            CAT,
            obj = pad,
            "Handling {:?} run ts start {} end {} pad active {}",
            buffer,
            start_running_time.display(),
            end_running_time.display(),
            is_active
        );

        #[allow(clippy::blocks_in_conditions)]
        let output_clockid = if is_active {
            pad_state.schedule_clock(
                self,
                pad,
                start_running_time,
                state.upstream_latency + settings.latency,
            )
        } else if state.output_running_time.is_some()
            && end_running_time.is_some_and(|end_running_time| {
                end_running_time < state.output_running_time.unwrap()
            })
        {
            if raw_pad {
                log!(
                    CAT,
                    obj = pad,
                    "Dropping trailing raw {:?} before timeout {}",
                    buffer,
                    state.timeout_running_time.unwrap()
                );
                return Ok(gst::FlowSuccess::Ok);
            } else {
                log!(
                    CAT,
                    obj = pad,
                    "Not dropping trailing non-raw {:?} before timeout {}",
                    buffer,
                    state.timeout_running_time.unwrap()
                );

                None
            }
        } else {
            pad_state.schedule_clock(
                self,
                pad,
                end_running_time,
                state.upstream_latency + settings.timeout + settings.latency,
            )
        };

        drop(pad_state);

        let mut update_all_pad_health = false;

        /* Before sleeping, ensure there is a timeout to switch active pads,
         * in case the initial active pad never receives a buffer */
        if let Some(running_time) = start_running_time {
            if state.timeout_clock_id.is_none() && !is_active {
                // May change active pad immediately
                update_all_pad_health = self.schedule_timeout(&mut state, &settings, running_time);
                is_active = self.active_sinkpad.lock().as_ref() == Some(pad);
            }
        }

        if let Some(clock_id) = &output_clockid {
            MutexGuard::unlocked(&mut state, || {
                let (_res, _) = clock_id.wait();
            });

            is_active = self.active_sinkpad.lock().as_ref() == Some(pad);
        }

        let pad_state = pad_imp.state.lock();
        if pad_state.flushing {
            debug!(CAT, imp = self, "Flushing");
            return Err(gst::FlowError::Flushing);
        }
        // calling schedule_timeout() may result in handle_timeout() being called right away,
        // which will need pad state locks, so drop it now to prevent deadlocks.
        drop(pad_state);

        if is_active {
            if start_running_time
                .opt_lt(state.output_running_time)
                .unwrap_or(false)
            {
                if raw_pad {
                    log!(
                        CAT,
                        obj = pad,
                        "Dropping trailing raw {:?} before output running time {}",
                        buffer,
                        state.output_running_time.display(),
                    );
                    return Ok(gst::FlowSuccess::Ok);
                } else {
                    log!(
                        CAT,
                        obj = pad,
                        "Not dropping trailing non-raw {:?} before output running time {}",
                        buffer,
                        state.output_running_time.display(),
                    );
                }
            }

            if let Some(start_running_time) = start_running_time {
                if let Some(output_running_time) = state.output_running_time {
                    state.output_running_time =
                        Some(std::cmp::max(start_running_time, output_running_time));
                } else {
                    state.output_running_time = Some(start_running_time);
                }
            }

            if let Some(end_running_time) = end_running_time {
                // May change active pad immediately
                update_all_pad_health |=
                    self.schedule_timeout(&mut state, &settings, end_running_time);
                is_active = self.active_sinkpad.lock().as_ref() == Some(pad);
            } else {
                state.cancel_timeout();
            }
        }

        let mut pad_state = pad_imp.state.lock();

        if let Some(running_time) = end_running_time {
            pad_state.current_running_time = Some(running_time);
        }
        let is_healthy = pad_state.is_healthy(pad, &state, &settings, state.output_running_time);
        let health_changed = is_healthy != pad_state.is_healthy;
        if health_changed {
            log!(CAT, obj = pad, "Health changed to {}", is_healthy);
        }
        pad_state.is_healthy = is_healthy;
        drop(pad_state);

        /* If the schedule_timeout() calls above said the timeout happened,
         * we should update the health of all pads here */
        let mut state = if update_all_pad_health {
            let changed_health_pads = self.update_health_statuses(&state, &settings);
            drop(state);

            for pad in changed_health_pads {
                pad.notify(PROP_IS_HEALTHY);
            }

            self.state.lock()
        } else {
            state
        };

        if !is_active {
            log!(CAT, obj = pad, "Dropping {:?} on inactive pad", buffer);

            drop(state);
            if health_changed {
                pad.notify(PROP_IS_HEALTHY);
            }

            return Ok(gst::FlowSuccess::Ok);
        }

        // Lock order: First stream lock then state lock!
        let _stream_lock = MutexGuard::unlocked(&mut state, || self.src_pad.stream_lock());

        is_active = self.active_sinkpad.lock().as_ref() == Some(pad);
        if !is_active {
            log!(CAT, obj = pad, "Dropping {:?} on inactive pad", buffer);

            drop(state);
            if health_changed {
                pad.notify(PROP_IS_HEALTHY);
            }
            return Ok(gst::FlowSuccess::Ok);
        }

        /* Update the health status for all pads, since we're the active pad */
        let changed_health_pads = self.update_health_statuses(&state, &settings);

        let switched_pad = state.switched_pad;
        let discont_pending = state.discont_pending;
        state.switched_pad = false;
        state.discont_pending = false;
        drop(state);

        if health_changed {
            pad.notify(PROP_IS_HEALTHY);
        }
        for pad in changed_health_pads {
            pad.notify(PROP_IS_HEALTHY);
        }

        if switched_pad {
            self.with_src_busy(|| {
                let _ = pad.push_event(gst::event::Reconfigure::new());
                pad.sticky_events_foreach(|event| {
                    self.src_pad.push_event(event.clone());
                    std::ops::ControlFlow::Continue(gst::EventForeachAction::Keep)
                });
            });

            self.obj().notify(PROP_ACTIVE_PAD);
        }

        if discont_pending && !buffer.flags().contains(gst::BufferFlags::DISCONT) {
            let buffer = buffer.make_mut();
            buffer.set_flags(gst::BufferFlags::DISCONT);
        }

        /* TODO: Clip raw video and audio buffers to avoid going backward? */

        log!(CAT, obj = pad, "Forwarding {:?}", buffer);

        if let Some(in_gap_event) = from_gap {
            // Safe unwrap: the buffer was constructed from a gap event with
            // a timestamp, and even if its timestamp was adjusted it should never
            // be NONE by now
            let pts = buffer.pts().unwrap();

            let out_gap_event = {
                #[cfg(feature = "v1_20")]
                {
                    gst::event::Gap::builder(pts)
                        .duration(buffer.duration())
                        .seqnum(in_gap_event.seqnum())
                        .gap_flags(in_gap_event.gap_flags())
                        .build()
                }
                #[cfg(not(feature = "v1_20"))]
                {
                    gst::event::Gap::builder(pts)
                        .duration(buffer.duration())
                        .seqnum(in_gap_event.seqnum())
                        .build()
                }
            };

            self.with_src_busy(|| {
                self.src_pad.push_event(out_gap_event);
            });

            Ok(gst::FlowSuccess::Ok)
        } else {
            self.with_src_busy(|| self.src_pad.push(buffer))
        }
    }

    fn sink_chain_list(
        &self,
        pad: &super::FallbackSwitchSinkPad,
        list: gst::BufferList,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        log!(CAT, obj = pad, "Handling buffer list {:?}", list);
        // TODO: Keep the list intact and forward it in one go (or broken into several
        // pieces if needed) when outputting to the active pad
        for buffer in list.iter_owned() {
            self.chain(pad, buffer, None)?;
        }

        Ok(gst::FlowSuccess::Ok)
    }

    fn sink_event(&self, pad: &super::FallbackSwitchSinkPad, event: gst::Event) -> bool {
        log!(CAT, obj = pad, "Handling event {:?}", event);

        if let gst::EventView::Gap(ev) = event.view() {
            let mut buffer = gst::Buffer::new();

            {
                let buf_mut = buffer.get_mut().unwrap();
                buf_mut.set_flags(gst::BufferFlags::GAP);
                let (pts, duration) = ev.get();
                buf_mut.set_pts(pts);
                buf_mut.set_duration(duration);
            }

            return match self.chain(pad, buffer, Some(ev)) {
                Ok(_) => true,
                Err(gst::FlowError::Flushing) | Err(gst::FlowError::Eos) => true,
                Err(err) => {
                    gst::error!(CAT, obj = pad, "Error processing gap event: {}", err);
                    false
                }
            };
        }

        let mut state = self.state.lock();

        let mut pad_state = pad.imp().state.lock();

        match event.view() {
            gst::EventView::Caps(caps) => {
                let caps = caps.caps();
                debug!(CAT, obj = pad, "Received caps {}", caps);

                let caps_info = match caps.structure(0).unwrap().name().as_str() {
                    "audio/x-raw" => {
                        CapsInfo::Audio(gst_audio::AudioInfo::from_caps(caps).unwrap())
                    }
                    "video/x-raw" => {
                        CapsInfo::Video(gst_video::VideoInfo::from_caps(caps).unwrap())
                    }
                    _ => CapsInfo::None,
                };

                pad_state.caps_info = caps_info;
            }
            gst::EventView::Segment(e) => {
                let segment = match e.segment().clone().downcast::<gst::ClockTime>() {
                    Err(segment) => {
                        gst::element_imp_error!(
                            self,
                            gst::StreamError::Format,
                            ["Only TIME segments supported, got {:?}", segment.format(),]
                        );
                        return false;
                    }
                    Ok(segment) => segment,
                };

                pad_state.segment = segment;
            }
            gst::EventView::FlushStart(_) => {
                pad_state.flush_start();
            }
            gst::EventView::FlushStop(_) => {
                pad_state.reset();
                state.first = true;
            }
            gst::EventView::Eos(_) => {
                pad_state.eos = true;
            }
            gst::EventView::StreamStart(_) => {
                pad_state.eos = false;
            }
            _ => {}
        }

        drop(pad_state);

        let mut is_active = self.active_sinkpad.lock().as_ref() == Some(pad);
        if !is_active {
            log!(CAT, obj = pad, "Dropping {:?} on inactive pad", event);
            return true;
        }

        // Lock order: First stream lock then state lock!
        let stream_lock_for_serialized = event
            .is_serialized()
            .then(|| MutexGuard::unlocked(&mut state, || self.src_pad.stream_lock()));

        is_active = self.active_sinkpad.lock().as_ref() == Some(pad);
        if !is_active {
            log!(CAT, obj = pad, "Dropping {:?} on inactive pad", event);
            return true;
        }

        let fwd_sticky = if state.switched_pad && stream_lock_for_serialized.is_some() {
            state.switched_pad = false;
            true
        } else {
            false
        };
        drop(state);

        if fwd_sticky {
            self.with_src_busy(|| {
                let _ = pad.push_event(gst::event::Reconfigure::new());
                pad.sticky_events_foreach(|event| {
                    self.src_pad.push_event(event.clone());
                    std::ops::ControlFlow::Continue(gst::EventForeachAction::Keep)
                });
            });

            self.obj().notify(PROP_ACTIVE_PAD);
        }

        self.with_src_busy(|| self.src_pad.push_event(event))
    }

    fn sink_query(&self, pad: &super::FallbackSwitchSinkPad, query: &mut gst::QueryRef) -> bool {
        use gst::QueryView;

        log!(CAT, obj = pad, "Handling query {:?}", query);

        let forward = match query.view() {
            QueryView::Context(_) => true,
            QueryView::Position(_) => true,
            QueryView::Duration(_) => true,
            QueryView::Caps(_) => true,
            QueryView::Allocation(_) => {
                /* Forward allocation only for the active sink pad,
                 * for others switching will send a reconfigure event upstream
                 */
                self.active_sinkpad.lock().as_ref() == Some(pad)
            }
            _ => {
                gst::Pad::query_default(pad, Some(&*self.obj()), query);
                false
            }
        };

        if forward {
            log!(CAT, obj = pad, "Forwarding query {:?}", query);
            self.src_pad.peer_query(query)
        } else {
            false
        }
    }

    fn reset(&self) {
        let mut state = self.state.lock();
        *state = State::default();
        self.active_sinkpad.lock().take();
    }

    fn src_query(&self, pad: &gst::Pad, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        log!(CAT, obj = pad, "Handling {:?}", query);

        match query.view_mut() {
            QueryViewMut::Latency(ref mut q) => {
                let mut ret = true;
                let mut min_latency = gst::ClockTime::ZERO;
                let mut max_latency = gst::ClockTime::NONE;

                for pad in self.obj().sink_pads() {
                    let mut peer_query = gst::query::Latency::new();

                    ret = pad.peer_query(&mut peer_query);

                    if ret {
                        let (live, min, max) = peer_query.result();
                        if live {
                            min_latency = min.max(min_latency);
                            max_latency = max
                                .zip(max_latency)
                                .map(|(max, max_latency)| max.min(max_latency))
                                .or(max);
                        }
                    }
                }

                let settings = self.settings.lock().clone();
                let mut state = self.state.lock();
                min_latency = min_latency.max(settings.min_upstream_latency);
                state.upstream_latency = min_latency;
                log!(CAT, obj = pad, "Upstream latency {}", min_latency);

                q.set(true, min_latency + settings.latency, max_latency);

                ret
            }
            QueryViewMut::Caps(_) => {
                // Unlock before forwarding
                let sinkpad = self.active_sinkpad.lock().clone();

                if let Some(sinkpad) = sinkpad {
                    sinkpad.peer_query(query)
                } else {
                    gst::Pad::query_default(pad, Some(&*self.obj()), query)
                }
            }
            _ => {
                // Unlock before forwarding
                let sinkpad = self.active_sinkpad.lock().clone();

                if let Some(sinkpad) = sinkpad {
                    sinkpad.peer_query(query)
                } else {
                    true
                }
            }
        }
    }

    /// check if at least one sink pad has received eos
    fn has_sink_pad_eos(&self) -> bool {
        let pads = self.obj().sink_pads();

        for pad in pads {
            let pad = pad.downcast_ref::<super::FallbackSwitchSinkPad>().unwrap();
            let pad_imp = pad.imp();
            let pad_state = pad_imp.state.lock();
            if pad_state.eos {
                return true;
            }
        }

        false
    }

    /// Wait until src_busy is not set and set it, execute
    /// the closure, then unset it again and notify its Cond.
    ///
    /// The State lock is taken while modifying src_busy,
    /// but not while executing the closure.
    fn with_src_busy<F, R>(&self, func: F) -> R
    where
        F: FnOnce() -> R,
    {
        {
            let mut state = self.state.lock();
            while state.src_busy {
                self.src_busy_cond.wait(&mut state);
            }
            state.src_busy = true;
        }

        let ret = func();

        {
            let mut state = self.state.lock();
            state.src_busy = false;
            self.src_busy_cond.notify_one();
        }

        ret
    }
}

#[glib::object_subclass]
impl ObjectSubclass for FallbackSwitch {
    const NAME: &'static str = "GstFallbackSwitch";
    type Type = super::FallbackSwitch;
    type ParentType = gst::Element;
    type Interfaces = (gst::ChildProxy,);

    fn with_class(klass: &Self::Class) -> Self {
        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_from_template(&templ)
            .query_function(|pad, parent, query| {
                FallbackSwitch::catch_panic_pad_function(
                    parent,
                    || false,
                    |fallbackswitch| fallbackswitch.src_query(pad, query),
                )
            })
            .build();

        Self {
            state: Mutex::new(State::default()),
            src_busy_cond: Condvar::default(),
            settings: Mutex::new(Settings::default()),
            active_sinkpad: Mutex::new(None),
            src_pad: srcpad,
            sink_pad_serial: AtomicU32::new(0),
        }
    }
}

impl ObjectImpl for FallbackSwitch {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecObject::builder::<gst::Pad>(PROP_ACTIVE_PAD)
                    .nick("Active Pad")
                    .blurb("Currently active pad")
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt64::builder(PROP_TIMEOUT)
                    .nick("Input timeout")
                    .blurb("Timeout on an input before switching to a lower priority input.")
                    .maximum(u64::MAX - 1)
                    .default_value(Settings::default().timeout.nseconds())
                    .mutable_playing()
                    .build(),
                glib::ParamSpecUInt64::builder(PROP_LATENCY)
                    .nick("Latency")
                    .blurb("Additional latency in live mode to allow upstream to take longer to produce buffers for the current position (in nanoseconds)")
                    .maximum(u64::MAX - 1)
                    .default_value(Settings::default().latency.nseconds())
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt64::builder(PROP_MIN_UPSTREAM_LATENCY)
                    .nick("Minimum Upstream Latency")
                    .blurb("When sources with a higher latency are expected to be plugged in dynamically after the fallbackswitch has started playing, this allows overriding the minimum latency reported by the initial source(s). This is only taken into account when larger than the actually reported minimum latency. (nanoseconds)")
                    .maximum(u64::MAX - 1)
                    .default_value(Settings::default().min_upstream_latency.nseconds())
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder(PROP_IMMEDIATE_FALLBACK)
                    .nick("Immediate fallback")
                    .blurb("Forward lower-priority streams immediately at startup, when the stream with priority 0 is slow to start up and immediate output is required")
                    .default_value(Settings::default().immediate_fallback)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder(PROP_AUTO_SWITCH)
                    .nick("Automatically switch pads")
                    .blurb("Automatically switch pads (If true, use the priority pad property, otherwise manual selection via the active-pad property)")
                    .default_value(Settings::default().auto_switch)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder(PROP_STOP_ON_EOS)
                    .nick("stop on EOS")
                    .blurb("Stop forwarding buffers as soon as one input pad is eos")
                    .default_value(Settings::default().stop_on_eos)
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            PROP_ACTIVE_PAD => {
                let settings = self.settings.lock();
                if settings.auto_switch {
                    gst::warning!(
                        CAT,
                        imp = self,
                        "active-pad property setting ignored, because auto-switch=true"
                    );
                } else {
                    let active_pad = value
                        .get::<Option<gst::Pad>>()
                        .expect("type checked upstream");
                    /* Trigger a pad switch if needed */
                    if let Some(active_pad) = active_pad {
                        self.set_active_pad(
                            &mut self.state.lock(),
                            active_pad
                                .downcast_ref::<super::FallbackSwitchSinkPad>()
                                .unwrap(),
                        );
                    }
                }
                drop(settings);
            }
            PROP_TIMEOUT => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");

                settings.timeout = new_value;
                debug!(CAT, imp = self, "Timeout now {}", settings.timeout);
                drop(settings);
                let _ = self
                    .obj()
                    .post_message(gst::message::Latency::builder().src(&*self.obj()).build());
            }
            PROP_LATENCY => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");

                settings.latency = new_value;
                drop(settings);
                let _ = self
                    .obj()
                    .post_message(gst::message::Latency::builder().src(&*self.obj()).build());
            }
            PROP_MIN_UPSTREAM_LATENCY => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");

                settings.min_upstream_latency = new_value;
                drop(settings);
                let _ = self
                    .obj()
                    .post_message(gst::message::Latency::builder().src(&*self.obj()).build());
            }
            PROP_IMMEDIATE_FALLBACK => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");
                settings.immediate_fallback = new_value;
            }
            PROP_AUTO_SWITCH => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");
                settings.auto_switch = new_value;
            }
            PROP_STOP_ON_EOS => {
                let mut settings = self.settings.lock();
                let new_value = value.get().expect("type checked upstream");
                settings.stop_on_eos = new_value;
            }
            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            PROP_ACTIVE_PAD => {
                let active_pad = self.active_sinkpad.lock().clone();
                active_pad.to_value()
            }
            PROP_TIMEOUT => {
                let settings = self.settings.lock();
                settings.timeout.to_value()
            }
            PROP_LATENCY => {
                let settings = self.settings.lock();
                settings.latency.to_value()
            }
            PROP_MIN_UPSTREAM_LATENCY => {
                let settings = self.settings.lock();
                settings.min_upstream_latency.to_value()
            }
            PROP_IMMEDIATE_FALLBACK => {
                let settings = self.settings.lock();
                settings.immediate_fallback.to_value()
            }
            PROP_AUTO_SWITCH => {
                let settings = self.settings.lock();
                settings.auto_switch.to_value()
            }
            PROP_STOP_ON_EOS => {
                let settings = self.settings.lock();
                settings.stop_on_eos.to_value()
            }
            _ => unimplemented!(),
        }
    }

    fn constructed(&self) {
        self.parent_constructed();

        let obj = self.obj();
        obj.add_pad(&self.src_pad).unwrap();
        obj.set_element_flags(gst::ElementFlags::REQUIRE_CLOCK);
    }
}

impl ElementImpl for FallbackSwitch {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Priority-based input selector",
                "Generic",
                "Priority-based automatic input selector element",
                "Jan Schmidt <jan@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let caps = gst::Caps::new_any();
            let sink_pad_template = gst::PadTemplate::with_gtype(
                "sink_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &caps,
                super::FallbackSwitchSinkPad::static_type(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![sink_pad_template, src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        trace!(CAT, imp = self, "Changing state {:?}", transition);

        match transition {
            gst::StateChange::PlayingToPaused => {
                self.cancel_waits();
            }
            gst::StateChange::ReadyToNull => {
                self.reset();
            }
            gst::StateChange::ReadyToPaused => {
                let mut state = self.state.lock();
                let prev_active_pad = self.active_sinkpad.lock().take();
                *state = State::default();
                let pads = self.obj().sink_pads();

                if let Some(pad) = pads.first() {
                    let pad = pad.downcast_ref::<super::FallbackSwitchSinkPad>().unwrap();

                    *self.active_sinkpad.lock() = Some(pad.clone());
                    state.switched_pad = true;
                    state.discont_pending = true;
                    drop(state);

                    if prev_active_pad.as_ref() != Some(pad) {
                        self.obj().notify(PROP_ACTIVE_PAD);
                    }
                }
                for pad in pads {
                    let pad = pad.downcast_ref::<super::FallbackSwitchSinkPad>().unwrap();
                    let pad_imp = pad.imp();
                    *pad_imp.state.lock() = SinkState::default();
                }
            }
            _ => (),
        }

        let mut success = self.parent_change_state(transition)?;

        match transition {
            gst::StateChange::ReadyToPaused => {
                success = gst::StateChangeSuccess::NoPreroll;
            }
            gst::StateChange::PlayingToPaused => {
                success = gst::StateChangeSuccess::NoPreroll;
            }
            gst::StateChange::PausedToReady => {
                *self.state.lock() = State::default();
                for pad in self.obj().sink_pads() {
                    let pad = pad.downcast_ref::<super::FallbackSwitchSinkPad>().unwrap();
                    let pad_imp = pad.imp();
                    *pad_imp.state.lock() = SinkState::default();
                }
            }
            _ => (),
        }

        Ok(success)
    }

    fn request_new_pad(
        &self,
        templ: &gst::PadTemplate,
        _name: Option<&str>,
        _caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        let mut state = self.state.lock();

        let pad_serial = self.sink_pad_serial.fetch_add(1, Ordering::SeqCst);

        let pad = gst::PadBuilder::<super::FallbackSwitchSinkPad>::from_template(templ)
            .name(format!("sink_{pad_serial}").as_str())
            .chain_function(|pad, parent, buffer| {
                FallbackSwitch::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |fallbackswitch| fallbackswitch.sink_chain(pad, buffer),
                )
            })
            .chain_list_function(|pad, parent, bufferlist| {
                FallbackSwitch::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |fallbackswitch| fallbackswitch.sink_chain_list(pad, bufferlist),
                )
            })
            .event_function(|pad, parent, event| {
                FallbackSwitch::catch_panic_pad_function(
                    parent,
                    || false,
                    |fallbackswitch| fallbackswitch.sink_event(pad, event),
                )
            })
            .query_function(|pad, parent, query| {
                FallbackSwitch::catch_panic_pad_function(
                    parent,
                    || false,
                    |fallbackswitch| fallbackswitch.sink_query(pad, query),
                )
            })
            .activatemode_function(|pad, _parent, mode, activate| {
                Self::sink_activatemode(pad, mode, activate)
            })
            .build();

        pad.set_active(true).unwrap();
        self.obj().add_pad(&pad).unwrap();

        let notify_active_pad = match &mut *self.active_sinkpad.lock() {
            active_sinkpad @ None => {
                *active_sinkpad = Some(pad.clone());
                state.switched_pad = true;
                state.discont_pending = true;
                true
            }
            _ => false,
        };

        let mut pad_settings = pad.imp().settings.lock();
        pad_settings.priority = pad_serial;
        drop(pad_settings);
        drop(state);

        if notify_active_pad {
            self.obj().notify(PROP_ACTIVE_PAD);
        }

        let _ = self
            .obj()
            .post_message(gst::message::Latency::builder().src(&*self.obj()).build());

        self.obj().child_added(&pad, &pad.name());
        Some(pad.upcast())
    }

    fn release_pad(&self, pad: &gst::Pad) {
        let pad = pad.downcast_ref::<super::FallbackSwitchSinkPad>().unwrap();
        let mut pad_state = pad.imp().state.lock();
        pad_state.flush_start();
        drop(pad_state);

        let _ = pad.set_active(false);
        self.obj().remove_pad(pad).unwrap();

        self.obj().child_removed(pad, &pad.name());
        let _ = self
            .obj()
            .post_message(gst::message::Latency::builder().src(&*self.obj()).build());
    }
}

// Implementation of gst::ChildProxy virtual methods.
//
// This allows accessing the pads and their properties from e.g. gst-launch.
impl ChildProxyImpl for FallbackSwitch {
    fn children_count(&self) -> u32 {
        let object = self.obj();
        object.num_pads() as u32
    }

    fn child_by_name(&self, name: &str) -> Option<glib::Object> {
        let object = self.obj();
        object
            .pads()
            .into_iter()
            .find(|p| p.name() == name)
            .map(|p| p.upcast())
    }

    fn child_by_index(&self, index: u32) -> Option<glib::Object> {
        let object = self.obj();
        object
            .pads()
            .into_iter()
            .nth(index as usize)
            .map(|p| p.upcast())
    }
}
