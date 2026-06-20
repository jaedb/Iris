// SPDX-License-Identifier: MIT OR Apache-2.0
// This is based on https://github.com/smol-rs/async-io
// with adaptations by:
//
// Copyright (C) 2021-2022 François Laignel <fengalin@free.fr>

use futures::stream::{FusedStream, Stream};

use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use super::reactor::{AfterTimerId, Reactor, RegularTimerId};

#[derive(Debug)]
pub struct IntervalError;

impl Error for IntervalError {}

impl fmt::Display for IntervalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Interval period can't be null")
    }
}

/// Creates a timer that emits an event once after the given delay.
///
/// When throttling is activated (i.e. when using a non-`0` `wait`
/// duration in `Context::acquire`), timer entries are assigned to
/// the nearest time frame, meaning that the delay might elapse
/// `wait` / 2 ms earlier or later than the expected instant.
///
/// Use [`delay_for_at_least`] when it's preferable not to return
/// before the expected instant.
pub fn delay_for(delay: Duration) -> Oneshot {
    if delay <= Reactor::with(|r| r.half_max_throttling()) {
        // timer should fire now.
        return Oneshot::new(Reactor::with(|r| r.timers_check_instant()));
    }

    Oneshot::new(Instant::now() + delay)
}

/// Creates a timer that emits an event once at the given time instant.
///
/// When throttling is activated (i.e. when using a non-`0` `wait`
/// duration in `Context::acquire`), timer entries are assigned to
/// the nearest time frame, meaning that the delay might elapse
/// `wait` / 2 ms earlier or later than the expected instant.
///
/// Use [`after`] when it's preferable not to return
/// before the expected instant.
pub fn at(when: Instant) -> Oneshot {
    if when <= Instant::now() {
        // timer should fire now.
        return Oneshot::new(Reactor::with(|r| r.timers_check_instant()));
    }

    Oneshot::new(when)
}

/// Creates a timer that emits events periodically, starting as soon as possible.
///
/// Returns an error if `period` is zero.
///
/// When throttling is activated (i.e. when using a non-`0` `wait`
/// duration in `Context::acquire`), timer entries are assigned to
/// the nearest time frame, meaning that the delay might elapse
/// `wait` / 2 ms earlier or later than the expected instant.
///
/// Use [`interval_at_least`] when it's preferable not to tick
/// before the expected instants.
pub fn interval(period: Duration) -> Result<Interval, IntervalError> {
    interval_at(Instant::now(), period)
}

/// Creates a timer that emits events periodically, starting after `delay`.
///
/// Returns an error if `period` is zero.
///
/// When throttling is activated (i.e. when using a non-`0` `wait`
/// duration in `Context::acquire`), timer entries are assigned to
/// the nearest time frame, meaning that the delay might elapse
/// `wait` / 2 ms earlier or later than the expected instant.
///
/// Use [`interval_delayed_by_at_least`] when it's preferable not to tick
/// before the expected instants.
pub fn interval_delayed_by(delay: Duration, period: Duration) -> Result<Interval, IntervalError> {
    interval_at(Instant::now() + delay, period)
}

/// Creates a timer that emits events periodically, starting at `start`.
///
/// When throttling is activated (i.e. when using a non-`0` `wait`
/// duration in `Context::acquire`), timer entries are assigned to
/// the nearest time frame, meaning that the delay might elapse
/// `wait` / 2 ms earlier or later than the expected instant.
///
/// Use [`interval_after_at_least`] when it's preferable not to tick
/// before the expected instants.
pub fn interval_at(start: Instant, period: Duration) -> Result<Interval, IntervalError> {
    if period.is_zero() {
        return Err(IntervalError);
    }

    Ok(Interval::new(start, period))
}

/// Creates a timer that emits an event once after the given delay.
///
/// See [`delay_for`] for details. The event is guaranteed to be emitted
/// no sooner than the expected delay has elapsed.
#[track_caller]
pub fn delay_for_at_least(delay: Duration) -> OneshotAfter {
    if delay.is_zero() {
        // timer should fire now.
        return OneshotAfter::new(Reactor::with(|r| r.timers_check_instant()));
    }

    OneshotAfter::new(Instant::now() + delay)
}

/// Creates a timer that emits an event once no sooner than the given time instant.
///
/// See [`at`] for details.
#[track_caller]
pub fn after(when: Instant) -> OneshotAfter {
    if when <= Instant::now() {
        // timer should fire now.
        return OneshotAfter::new(Reactor::with(|r| r.timers_check_instant()));
    }

    OneshotAfter::new(when)
}

/// Creates a timer that emits events periodically, starting as soon as possible.
///
/// Returns an error if `period` is zero.
///
/// See [`interval`] for details. The events are guaranteed to be
/// emitted no sooner than the expected instants.
pub fn interval_at_least(period: Duration) -> Result<IntervalAfter, IntervalError> {
    interval_after_at_least(Instant::now(), period)
}

/// Creates a timer that emits events periodically, starting after at least `delay`.
///
/// Returns an error if `period` is zero.
///
/// See [`interval_delayed_by`] for details. The events are guaranteed to be
/// emitted no sooner than the expected instants.
#[track_caller]
pub fn interval_delayed_by_at_least(
    delay: Duration,
    period: Duration,
) -> Result<IntervalAfter, IntervalError> {
    interval_after_at_least(Instant::now() + delay, period)
}

/// Creates a timer that emits events periodically, starting at `start`.
///
/// See [`interval_at`] for details. The events are guaranteed to be
/// emitted no sooner than the expected instants.
#[track_caller]
pub fn interval_after_at_least(
    start: Instant,
    period: Duration,
) -> Result<IntervalAfter, IntervalError> {
    if period.is_zero() {
        return Err(IntervalError);
    }

    Ok(IntervalAfter::new(start, period))
}

/// A future that emits an event at the given time.
///
/// `Oneshot`s are futures that resolve when the given time is reached.
///
/// When throttling is activated (i.e. when using a non-`0` `wait`
/// duration in `Context::acquire`), timer entries are assigned to
/// the nearest time frame, meaning that the timer may fire
/// `wait` / 2 ms earlier or later than the expected instant.
#[derive(Debug)]
pub struct Oneshot {
    /// This timer's ID and last waker that polled it.
    ///
    /// When this field is set to `None`, this timer is not registered in the reactor.
    id_and_waker: Option<(RegularTimerId, Waker)>,

    /// The instant at which this timer fires.
    when: Instant,
}

impl Oneshot {
    fn new(when: Instant) -> Self {
        Oneshot {
            id_and_waker: None,
            when,
        }
    }
}

impl Drop for Oneshot {
    fn drop(&mut self) {
        if let Some((id, _)) = self.id_and_waker.take() {
            Reactor::with_mut(|reactor| {
                reactor.remove_timer(self.when, id);
            });
        }
    }
}

impl Future for Oneshot {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Reactor::with_mut(|reactor| {
            if reactor.time_slice_end() >= self.when {
                if let Some((id, _)) = self.id_and_waker.take() {
                    // Deregister the timer from the reactor.
                    reactor.remove_timer(self.when, id);
                }

                Poll::Ready(())
            } else {
                match &self.id_and_waker {
                    None => {
                        // Register the timer in the reactor.
                        let id = reactor.insert_regular_timer(self.when, cx.waker());
                        self.id_and_waker = Some((id, cx.waker().clone()));
                    }
                    Some((id, w)) if !w.will_wake(cx.waker()) => {
                        // Deregister the timer from the reactor to remove the old waker.
                        reactor.remove_timer(self.when, *id);

                        // Register the timer in the reactor with the new waker.
                        let id = reactor.insert_regular_timer(self.when, cx.waker());
                        self.id_and_waker = Some((id, cx.waker().clone()));
                    }
                    Some(_) => {}
                }

                Poll::Pending
            }
        })
    }
}

/// A future that emits an event at the given time.
///
/// `OneshotAfter`s are futures that always resolve after
/// the given time is reached.
#[derive(Debug)]
pub struct OneshotAfter {
    /// This timer's ID and last waker that polled it.
    ///
    /// When this field is set to `None`, this timer is not registered in the reactor.
    id_and_waker: Option<(AfterTimerId, Waker)>,

    /// The instant at which this timer fires.
    when: Instant,
}

impl OneshotAfter {
    fn new(when: Instant) -> Self {
        OneshotAfter {
            id_and_waker: None,
            when,
        }
    }
}

impl Drop for OneshotAfter {
    fn drop(&mut self) {
        if let Some((id, _)) = self.id_and_waker.take() {
            Reactor::with_mut(|reactor| {
                reactor.remove_timer(self.when, id);
            });
        }
    }
}

impl Future for OneshotAfter {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Reactor::with_mut(|reactor| {
            if reactor.timers_check_instant() >= self.when {
                if let Some((id, _)) = self.id_and_waker.take() {
                    // Deregister the timer from the reactor.
                    reactor.remove_timer(self.when, id);
                }

                Poll::Ready(())
            } else {
                match &self.id_and_waker {
                    None => {
                        // Register the timer in the reactor.
                        let id = reactor.insert_after_timer(self.when, cx.waker());
                        self.id_and_waker = Some((id, cx.waker().clone()));
                    }
                    Some((id, w)) if !w.will_wake(cx.waker()) => {
                        // Deregister the timer from the reactor to remove the old waker.
                        reactor.remove_timer(self.when, *id);

                        // Register the timer in the reactor with the new waker.
                        let id = reactor.insert_after_timer(self.when, cx.waker());
                        self.id_and_waker = Some((id, cx.waker().clone()));
                    }
                    Some(_) => {}
                }

                Poll::Pending
            }
        })
    }
}

/// A stream that emits timed events.
///
/// `Interval`s are streams that ticks periodically in the closest
/// time slice.
#[derive(Debug)]
pub struct Interval {
    /// This timer's ID and last waker that polled it.
    ///
    /// When this field is set to `None`, this timer is not registered in the reactor.
    id_and_waker: Option<(RegularTimerId, Waker)>,

    /// The next instant at which this timer should fire.
    when: Instant,

    /// The period.
    period: Duration,
}

impl Interval {
    fn new(start: Instant, period: Duration) -> Self {
        Interval {
            id_and_waker: None,
            when: start,
            period,
        }
    }
}

impl Drop for Interval {
    fn drop(&mut self) {
        if let Some((id, _)) = self.id_and_waker.take() {
            Reactor::with_mut(|reactor| {
                reactor.remove_timer(self.when, id);
            });
        }
    }
}

impl Stream for Interval {
    type Item = ();

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Reactor::with_mut(|reactor| {
            let time_slice_end = reactor.time_slice_end();
            if time_slice_end >= self.when {
                if let Some((id, _)) = self.id_and_waker.take() {
                    // Deregister the timer from the reactor.
                    reactor.remove_timer(self.when, id);
                }
                // Compute the next tick making sure we are not so late
                // that we would need to tick again right now.
                let period = self.period;
                while time_slice_end >= self.when {
                    // This can't overflow in practical conditions.
                    self.when += period;
                }
                // Register the timer in the reactor.
                let id = reactor.insert_regular_timer(self.when, cx.waker());
                self.id_and_waker = Some((id, cx.waker().clone()));

                Poll::Ready(Some(()))
            } else {
                match &self.id_and_waker {
                    None => {
                        // Register the timer in the reactor.
                        let id = reactor.insert_regular_timer(self.when, cx.waker());
                        self.id_and_waker = Some((id, cx.waker().clone()));
                    }
                    Some((id, w)) if !w.will_wake(cx.waker()) => {
                        // Deregister the timer from the reactor to remove the old waker.
                        reactor.remove_timer(self.when, *id);

                        // Register the timer in the reactor with the new waker.
                        let id = reactor.insert_regular_timer(self.when, cx.waker());
                        self.id_and_waker = Some((id, cx.waker().clone()));
                    }
                    Some(_) => {}
                }

                Poll::Pending
            }
        })
    }
}

impl FusedStream for Interval {
    fn is_terminated(&self) -> bool {
        // Interval is "infinite" in practice
        false
    }
}
/// A stream that emits timed events.
///
/// `IntervalAfter`s are streams that ticks periodically. Ticks are
/// guaranteed to fire no sooner than the expected instant.
#[derive(Debug)]
pub struct IntervalAfter {
    /// This timer's ID and last waker that polled it.
    ///
    /// When this field is set to `None`, this timer is not registered in the reactor.
    id_and_waker: Option<(AfterTimerId, Waker)>,

    /// The next instant at which this timer should fire.
    when: Instant,

    /// The period.
    period: Duration,
}

impl IntervalAfter {
    fn new(start: Instant, period: Duration) -> Self {
        IntervalAfter {
            id_and_waker: None,
            when: start,
            period,
        }
    }
}

impl Drop for IntervalAfter {
    fn drop(&mut self) {
        if let Some((id, _)) = self.id_and_waker.take() {
            Reactor::with_mut(|reactor| {
                reactor.remove_timer(self.when, id);
            });
        }
    }
}

impl Stream for IntervalAfter {
    type Item = ();

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Reactor::with_mut(|reactor| {
            let timers_check_instant = reactor.timers_check_instant();
            if timers_check_instant >= self.when {
                if let Some((id, _)) = self.id_and_waker.take() {
                    // Deregister the timer from the reactor.
                    reactor.remove_timer(self.when, id);
                }
                // Compute the next tick making sure we are not so late
                // that we would need to tick again right now.
                let period = self.period;
                while timers_check_instant >= self.when {
                    // This can't overflow in practical conditions.
                    self.when += period;
                }
                // Register the timer in the reactor.
                let id = reactor.insert_after_timer(self.when, cx.waker());
                self.id_and_waker = Some((id, cx.waker().clone()));

                Poll::Ready(Some(()))
            } else {
                match &self.id_and_waker {
                    None => {
                        // Register the timer in the reactor.
                        let id = reactor.insert_after_timer(self.when, cx.waker());
                        self.id_and_waker = Some((id, cx.waker().clone()));
                    }
                    Some((id, w)) if !w.will_wake(cx.waker()) => {
                        // Deregister the timer from the reactor to remove the old waker.
                        reactor.remove_timer(self.when, *id);

                        // Register the timer in the reactor with the new waker.
                        let id = reactor.insert_after_timer(self.when, cx.waker());
                        self.id_and_waker = Some((id, cx.waker().clone()));
                    }
                    Some(_) => {}
                }

                Poll::Pending
            }
        })
    }
}

impl FusedStream for IntervalAfter {
    fn is_terminated(&self) -> bool {
        // IntervalAfter is "infinite" in practice
        false
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::runtime::executor::scheduler;

    const MAX_THROTTLING: Duration = Duration::from_millis(10);
    const DELAY: Duration = Duration::from_millis(12);
    const PERIOD: Duration = Duration::from_millis(15);

    #[test]
    fn delay_for_regular() {
        gst::init().unwrap();

        let handle = scheduler::Throttling::start("delay_for_regular", MAX_THROTTLING);

        futures::executor::block_on(handle.spawn(async {
            let start = Instant::now();
            super::delay_for(DELAY).await;
            // Due to throttling, timer may be fired earlier
            assert!(start.elapsed() + MAX_THROTTLING / 2 >= DELAY);
        }))
        .unwrap();
    }

    #[test]
    fn delay_for_at_least() {
        gst::init().unwrap();

        let handle = scheduler::Throttling::start("delay_for_at_least", MAX_THROTTLING);

        futures::executor::block_on(handle.spawn(async {
            let start = Instant::now();
            super::delay_for_at_least(DELAY).await;
            // Never returns earlier than DELAY
            assert!(start.elapsed() >= DELAY);
        }))
        .unwrap();
    }

    #[test]
    fn interval_regular() {
        use futures::prelude::*;

        gst::init().unwrap();

        let handle = scheduler::Throttling::start("interval_regular", MAX_THROTTLING);

        let join_handle = handle.spawn(async move {
            let mut acc = Duration::ZERO;

            let start = Instant::now();
            let mut interval = super::interval(PERIOD).unwrap();

            interval.next().await.unwrap();
            assert!(start.elapsed() + MAX_THROTTLING / 2 >= acc);

            // Due to throttling, intervals may tick earlier.
            for _ in 0..10 {
                interval.next().await.unwrap();
                acc += PERIOD;
                assert!(start.elapsed() + MAX_THROTTLING / 2 >= acc);
            }
        });

        futures::executor::block_on(join_handle).unwrap();
    }

    #[test]
    fn interval_after_at_least() {
        use futures::prelude::*;

        gst::init().unwrap();

        let handle = scheduler::Throttling::start("interval_after", MAX_THROTTLING);

        let join_handle = handle.spawn(async move {
            let mut acc = DELAY;

            let start = Instant::now();
            let mut interval = super::interval_after_at_least(start + DELAY, PERIOD).unwrap();

            interval.next().await.unwrap();
            assert!(start.elapsed() >= acc);

            for _ in 1..10 {
                interval.next().await.unwrap();
                acc += PERIOD;
                assert!(start.elapsed() >= acc);
            }
        });

        futures::executor::block_on(join_handle).unwrap();
    }
}
