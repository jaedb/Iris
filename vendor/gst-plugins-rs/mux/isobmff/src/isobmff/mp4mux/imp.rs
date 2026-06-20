use anyhow::{bail, Context};
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::prelude::*;
use gst_base::subclass::prelude::*;

use num_integer::Integer;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::str::FromStr;
use std::sync::Mutex;

use crate::av1::obu::read_seq_header_obu_bytes;
use crate::isobmff::boxes::create_dac3;
use crate::isobmff::boxes::create_dec3;
use crate::isobmff::boxes::create_ftyp;
use crate::isobmff::boxes::create_mdat_header_non_frag;
use crate::isobmff::boxes::create_moov;
use crate::isobmff::AuxiliaryInformation;
use crate::isobmff::AuxiliaryInformationEntry;
use crate::isobmff::Chunk;
use crate::isobmff::DeltaFrames;
use crate::isobmff::ElstInfo;
use crate::isobmff::PresentationConfiguration;
use crate::isobmff::Sample;
use crate::isobmff::TaiClockInfo;
use crate::isobmff::TaicClockType;
use crate::isobmff::TrackConfiguration;
use crate::isobmff::Variant;
use crate::isobmff::TAIC_TIME_UNCERTAINTY_UNKNOWN;
use std::sync::LazyLock;

use crate::isobmff::transform_matrix::TransformMatrix;

/// Offset between NTP and UNIX epoch in seconds.
/// NTP = UNIX + NTP_UNIX_OFFSET.
const NTP_UNIX_OFFSET: u64 = 2_208_988_800;

/// Reference timestamp meta caps for NTP timestamps.
static NTP_CAPS: LazyLock<gst::Caps> =
    LazyLock::new(|| gst::Caps::builder("timestamp/x-ntp").build());

/// Reference timestamp meta caps for UNIX timestamps.
static UNIX_CAPS: LazyLock<gst::Caps> =
    LazyLock::new(|| gst::Caps::builder("timestamp/x-unix").build());

#[cfg(feature = "v1_28")]
/// Reference timestamp meta caps for TAI timestamps with 1958-01-01 epoch.
static TAI1958_CAPS: LazyLock<gst::Caps> =
    LazyLock::new(|| gst::Caps::builder("timestamp/x-tai1958").build());

/// Returns the UTC time of the buffer in the UNIX epoch.
fn get_utc_time_from_buffer(buffer: &gst::BufferRef) -> Option<gst::ClockTime> {
    buffer
        .iter_meta::<gst::ReferenceTimestampMeta>()
        .find_map(|meta| {
            if meta.reference().can_intersect(&UNIX_CAPS) {
                Some(meta.timestamp())
            } else if meta.reference().can_intersect(&NTP_CAPS) {
                meta.timestamp().checked_sub(NTP_UNIX_OFFSET.seconds())
            } else {
                None
            }
        })
}

/// Converts a running time to an UTC time.
fn running_time_to_utc_time(
    running_time: impl Into<gst::Signed<gst::ClockTime>>,
    running_time_utc_time_mapping: (
        impl Into<gst::Signed<gst::ClockTime>>,
        impl Into<gst::Signed<gst::ClockTime>>,
    ),
) -> Option<gst::ClockTime> {
    running_time_utc_time_mapping
        .1
        .into()
        .checked_sub(running_time_utc_time_mapping.0.into())
        .and_then(|res| res.checked_add(running_time.into()))
        .and_then(|res| res.positive())
}

pub(crate) static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "mp4mux",
        gst::DebugColorFlags::empty(),
        Some("MP4Mux Element"),
    )
});

impl FromStr for TaicClockType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "unknown" => Ok(TaicClockType::Unknown),
            "cannot-sync-to-tai" => Ok(TaicClockType::CannotSync),
            "can-sync-to-tai" => Ok(TaicClockType::CanSync),
            _ => bail!("unknown TAI Clock type: {}", s),
        }
    }
}

const DEFAULT_INTERLEAVE_BYTES: Option<u64> = None;
const DEFAULT_INTERLEAVE_TIME: Option<gst::ClockTime> = Some(gst::ClockTime::from_mseconds(500));

#[derive(Debug, Clone)]
struct Settings {
    interleave_bytes: Option<u64>,
    interleave_time: Option<gst::ClockTime>,
    movie_timescale: u32,
    extra_brands: Vec<[u8; 4]>,
    with_precision_timestamps: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            interleave_bytes: DEFAULT_INTERLEAVE_BYTES,
            interleave_time: DEFAULT_INTERLEAVE_TIME,
            movie_timescale: 0,
            extra_brands: Vec::new(),
            with_precision_timestamps: false,
        }
    }
}

#[derive(Debug)]
struct PendingBuffer {
    buffer: gst::Buffer,
    timestamp: gst::Signed<gst::ClockTime>,
    pts: gst::ClockTime,
    composition_time_offset: Option<i64>,
    duration: Option<gst::ClockTime>,
}
#[derive(Debug)]
struct PendingAuxInfoEntry {
    aux_info_type: Option<[u8; 4]>,
    aux_info_type_parameter: u32,
    data: Vec<u8>,
}

#[derive(Debug)]
struct Stream {
    /// Sink pad for this stream.
    sinkpad: crate::isobmff::MP4MuxPad,

    /// Pre-queue for ONVIF variant to timestamp all buffers with their UTC time.
    pre_queue: VecDeque<(gst::FormattedSegment<gst::ClockTime>, gst::Buffer)>,

    /// Currently configured caps for this stream.
    caps: gst::Caps,
    /// Whether this stream is intra-only and has frame reordering.
    delta_frames: DeltaFrames,
    /// Whether this stream might have header frames without timestamps that should be ignored.
    discard_header_buffers: bool,

    /// Already written out chunks with their samples for this stream
    chunks: Vec<Chunk>,

    /// Queued time in the latest chunk.
    queued_chunk_time: gst::ClockTime,
    /// Queue bytes in the latest chunk.
    queued_chunk_bytes: u64,

    /// Currently pending buffer, DTS or PTS running time and duration
    ///
    /// If the duration is set then the next buffer is already queued up and the duration was
    /// calculated based on that.
    pending_buffer: Option<PendingBuffer>,

    /// Start DTS.
    start_dts: Option<gst::Signed<gst::ClockTime>>,

    /// Earliest PTS.
    earliest_pts: Option<gst::ClockTime>,

    /// Edit list entries for this stream.
    elst_infos: Vec<ElstInfo>,

    /// Current end PTS.
    end_pts: Option<gst::ClockTime>,

    /// In ONVIF mode, the mapping between running time and UTC time (UNIX)
    running_time_utc_time_mapping: Option<(gst::Signed<gst::ClockTime>, gst::ClockTime)>,

    extra_header_data: Option<Vec<u8>>,

    /// Codec-specific boxes to be included in the sample entry
    codec_specific_boxes: Vec<u8>,

    /// Language code from tags
    language_code: Option<[u8; 3]>,

    /// Orientation from tags, stream orientation takes precedence over global orientation
    global_orientation: &'static TransformMatrix,
    stream_orientation: Option<&'static TransformMatrix>,

    avg_bitrate: Option<u32>,
    max_bitrate: Option<u32>,

    /// TAI precision clock information
    tai_clock_info: Option<TaiClockInfo>,

    /// The auxiliary information (saio/saiz) for the stream
    aux_info: Vec<AuxiliaryInformation>,
    /// auxiliary information to be written after the chunk is finished
    pending_aux_info_data: VecDeque<PendingAuxInfoEntry>,
}

impl Stream {
    fn get_elst_infos(
        &self,
        min_earliest_pts: gst::ClockTime,
    ) -> Result<Vec<ElstInfo>, anyhow::Error> {
        let mut elst_infos = self.elst_infos.clone();
        let earliest_pts = self
            .earliest_pts
            .expect("Streams without earliest_pts should have been skipped");
        let end_pts = self
            .end_pts
            .expect("Streams without end_pts should have been skipped");

        // If no elst info were set, use the whole track
        if self.elst_infos.is_empty() {
            let start = if let Some(start_dts) = self.start_dts {
                gst::Signed::Positive(earliest_pts) - start_dts
            } else {
                gst::Signed::Positive(gst::ClockTime::ZERO)
            };

            elst_infos.push(ElstInfo {
                start: Some(start),
                duration: Some(end_pts - earliest_pts),
            });
        }

        // Add a gap at the beginning if needed
        if earliest_pts > min_earliest_pts {
            let gap_duration = earliest_pts - min_earliest_pts;

            elst_infos.insert(
                0,
                ElstInfo {
                    start: None,
                    duration: Some(gap_duration),
                },
            );
        }

        let mut iter = elst_infos
            .iter_mut()
            .filter(|e| e.start.is_some())
            .peekable();
        while let Some(&mut ref mut elst_info) = iter.next() {
            if elst_info.duration.is_none_or(|duration| duration.is_zero()) {
                elst_info.duration = if let Some(next) = iter.peek_mut() {
                    Some(
                        (next.start.unwrap() - elst_info.start.unwrap())
                            .positive()
                            .unwrap_or(gst::ClockTime::ZERO),
                    )
                } else {
                    Some(end_pts - earliest_pts)
                }
            }
        }

        Ok(elst_infos)
    }

    fn timescale(&self) -> u32 {
        let trak_timescale = { self.sinkpad.imp().settings.lock().unwrap().trak_timescale };

        if trak_timescale > 0 {
            return trak_timescale;
        }

        let s = self.caps.structure(0).unwrap();

        if let Ok(fps) = s.get::<gst::Fraction>("framerate") {
            if fps.numer() == 0 {
                return 10_000;
            }

            if fps.denom() != 1 && fps.denom() != 1001 {
                if let Some(fps) = (fps.denom() as u64)
                    .nseconds()
                    .mul_div_round(1_000_000_000, fps.numer() as u64)
                    .and_then(gst_video::guess_framerate)
                {
                    return (fps.numer() as u32)
                        .mul_div_round(100, fps.denom() as u32)
                        .unwrap_or(10_000);
                }
            }

            if fps.denom() == 1001 {
                fps.numer() as u32
            } else {
                (fps.numer() as u32)
                    .mul_div_round(100, fps.denom() as u32)
                    .unwrap_or(10_000)
            }
        } else if let Ok(rate) = s.get::<i32>("rate") {
            rate as u32
        } else {
            10_000
        }
    }

    fn image_sequence_mode(&self) -> bool {
        let image_sequence = {
            self.sinkpad
                .imp()
                .settings
                .lock()
                .unwrap()
                .image_sequence_mode
        };
        image_sequence
    }

    fn orientation(&self) -> &'static TransformMatrix {
        self.stream_orientation.unwrap_or(self.global_orientation)
    }

    fn parse_language_code(lang: &str) -> Option<[u8; 3]> {
        let lang = gst_tag::language_codes::language_code_iso_639_2t(lang)?;
        if lang.len() == 3 && lang.chars().all(|c| c.is_ascii_lowercase()) {
            let mut language_code: [u8; 3] = [0; 3];
            for (out, c) in Iterator::zip(language_code.iter_mut(), lang.chars()) {
                *out = c as u8;
            }
            Some(language_code)
        } else {
            None
        }
    }
}

#[derive(Default)]
struct State {
    /// List of streams when the muxer was started.
    streams: Vec<Stream>,

    /// Index of stream that is currently selected to fill a chunk.
    current_stream_idx: Option<usize>,

    /// Current writing offset since the beginning of the stream.
    current_offset: u64,

    /// Offset of the `mdat` box from the beginning of the stream.
    mdat_offset: Option<u64>,

    /// Size of the `mdat` as written so far.
    mdat_size: u64,

    #[cfg(feature = "v1_28")]
    /// The last TAI timestamp value, in nanoseconds after epoch
    last_tai_timestamp: u64,
}

#[derive(Default)]
pub(crate) struct MP4Mux {
    state: Mutex<State>,
    settings: Mutex<Settings>,
}

impl MP4Mux {
    /// Checks if a buffer is valid according to the stream configuration.
    fn check_buffer(
        buffer: &gst::BufferRef,
        sinkpad: &crate::isobmff::MP4MuxPad,
        delta_frames: DeltaFrames,
        discard_headers: bool,
    ) -> Result<(), gst::FlowError> {
        if discard_headers && buffer.flags().contains(gst::BufferFlags::HEADER) {
            return Err(gst_base::AGGREGATOR_FLOW_NEED_DATA);
        }

        if delta_frames.requires_dts() && buffer.dts().is_none() {
            gst::error!(CAT, obj = sinkpad, "Require DTS for video streams");
            return Err(gst::FlowError::Error);
        }

        if buffer.pts().is_none() {
            gst::error!(CAT, obj = sinkpad, "Require timestamped buffers");
            return Err(gst::FlowError::Error);
        }

        if delta_frames.intra_only() && buffer.flags().contains(gst::BufferFlags::DELTA_UNIT) {
            gst::error!(CAT, obj = sinkpad, "Intra-only stream with delta units");
            return Err(gst::FlowError::Error);
        }

        Ok(())
    }

    fn add_elst_info(
        &self,
        buffer: &PendingBuffer,
        stream: &mut Stream,
    ) -> Result<(), anyhow::Error> {
        let cmeta = if let Some(cmeta) = buffer.buffer.meta::<gst_audio::AudioClippingMeta>() {
            cmeta
        } else {
            return Ok(());
        };

        let timescale = stream
            .caps
            .structure(0)
            .unwrap()
            .get::<i32>("rate")
            .unwrap_or_else(|_| stream.timescale() as i32);

        let samples_to_gstclocktime = move |v: u64| {
            let nseconds = v
                .mul_div_round(gst::ClockTime::SECOND.nseconds(), timescale as u64)
                .context("Invalid start in the AudioClipMeta")?;
            Ok::<_, anyhow::Error>(gst::ClockTime::from_nseconds(nseconds))
        };

        let generic_to_gstclocktime = move |t| -> Result<Option<gst::ClockTime>, anyhow::Error> {
            if let gst::GenericFormattedValue::Default(Some(v)) = t {
                let v = u64::from(v);
                let v = samples_to_gstclocktime(v)?;
                Ok(Some(v).filter(|x| !x.is_zero()))
            } else if let gst::GenericFormattedValue::Time(Some(v)) = t {
                Ok(Some(v).filter(|x| !x.is_zero()))
            } else {
                Ok(None)
            }
        };

        let start = generic_to_gstclocktime(cmeta.start())?;
        let end = generic_to_gstclocktime(cmeta.end())?;

        if end.is_none() && start.is_none() {
            bail!("No start or end time in `default` format in the AudioClippingMeta");
        }

        let start = if let Some(start) = generic_to_gstclocktime(cmeta.start())? {
            start + buffer.pts
        } else {
            gst::ClockTime::ZERO
        };
        let duration = if let Some(end) = end {
            Some(
                buffer.pts
                    + buffer
                        .duration
                        .context("No duration on buffer, we can't add edit list")?
                    - end,
            )
        } else {
            None
        };

        stream.elst_infos.push(ElstInfo {
            start: Some(start.into()),
            duration,
        });

        Ok(())
    }

    fn peek_buffer(
        &self,
        sinkpad: &crate::isobmff::MP4MuxPad,
        delta_frames: DeltaFrames,
        discard_headers: bool,
        pre_queue: &mut VecDeque<(gst::FormattedSegment<gst::ClockTime>, gst::Buffer)>,
        running_time_utc_time_mapping: &Option<(gst::Signed<gst::ClockTime>, gst::ClockTime)>,
    ) -> Result<Option<(gst::FormattedSegment<gst::ClockTime>, gst::Buffer)>, gst::FlowError> {
        if let Some((segment, buffer)) = pre_queue.front() {
            return Ok(Some((segment.clone(), buffer.clone())));
        }

        let Some(mut buffer) = sinkpad.peek_buffer() else {
            return Ok(None);
        };
        Self::check_buffer(&buffer, sinkpad, delta_frames, discard_headers)?;
        let mut segment = match sinkpad.segment().downcast::<gst::ClockTime>().ok() {
            Some(segment) => segment,
            None => {
                gst::error!(CAT, obj = sinkpad, "Got buffer before segment");
                return Err(gst::FlowError::Error);
            }
        };

        // For ONVIF we need to re-timestamp the buffer with its UTC time.
        // We can only possibly end up here after the running-time UTC mapping is known.
        //
        // After re-timestamping, put the buffer into the pre-queue so re-timestamping only has to
        // happen once.
        if self.obj().class().as_ref().variant == Variant::ONVIF {
            let running_time_utc_time_mapping = running_time_utc_time_mapping.unwrap();

            let pts_position = buffer.pts().unwrap();
            let dts_position = buffer.dts();

            let pts = segment.to_running_time_full(pts_position).unwrap();

            let dts = dts_position
                .map(|dts_position| segment.to_running_time_full(dts_position).unwrap());

            let utc_time = match get_utc_time_from_buffer(&buffer) {
                None => {
                    // Calculate from the mapping
                    running_time_to_utc_time(pts, running_time_utc_time_mapping).ok_or_else(
                        || {
                            gst::error!(CAT, obj = sinkpad, "Stream has negative PTS UTC time");
                            gst::FlowError::Error
                        },
                    )?
                }
                Some(utc_time) => utc_time,
            };

            gst::trace!(
                CAT,
                obj = sinkpad,
                "Mapped PTS running time {pts} to UTC time {utc_time}"
            );

            {
                let buffer = buffer.make_mut();
                buffer.set_pts(utc_time);

                if let Some(dts) = dts {
                    let dts_utc_time =
                        running_time_to_utc_time(dts, (pts, utc_time)).ok_or_else(|| {
                            gst::error!(CAT, obj = sinkpad, "Stream has negative DTS UTC time");
                            gst::FlowError::Error
                        })?;
                    gst::trace!(
                        CAT,
                        obj = sinkpad,
                        "Mapped DTS running time {dts} to UTC time {dts_utc_time}"
                    );
                    buffer.set_dts(dts_utc_time);
                }
            }

            segment = gst::FormattedSegment::default();

            // Drop current buffer as it is now queued
            sinkpad.drop_buffer();
            pre_queue.push_back((segment.clone(), buffer.clone()));
        }

        Ok(Some((segment, buffer)))
    }

    fn pop_buffer(
        &self,
        stream: &mut Stream,
    ) -> Result<Option<(gst::FormattedSegment<gst::ClockTime>, gst::Buffer)>, gst::FlowError> {
        let Stream {
            sinkpad, pre_queue, ..
        } = stream;

        // In ONVIF mode we need to get UTC times for each buffer and synchronize based on that.
        // Queue up to 6s of data to get the first UTC time and then backdate.
        if self.obj().class().as_ref().variant == Variant::ONVIF
            && stream.running_time_utc_time_mapping.is_none()
        {
            if let Some((last, first)) = Option::zip(pre_queue.back(), pre_queue.front()) {
                // Existence of PTS/DTS checked below
                let (last, first) = if stream.delta_frames.requires_dts() {
                    (
                        last.0.to_running_time_full(last.1.dts()).unwrap(),
                        first.0.to_running_time_full(first.1.dts()).unwrap(),
                    )
                } else {
                    (
                        last.0.to_running_time_full(last.1.pts()).unwrap(),
                        first.0.to_running_time_full(first.1.pts()).unwrap(),
                    )
                };

                if last.saturating_sub(first)
                    > gst::Signed::Positive(gst::ClockTime::from_seconds(6))
                {
                    gst::error!(
                        CAT,
                        obj = sinkpad,
                        "Got no UTC time in the first 6s of the stream"
                    );
                    return Err(gst::FlowError::Error);
                }
            }

            let Some(buffer) = sinkpad.pop_buffer() else {
                if sinkpad.is_eos() {
                    gst::error!(CAT, obj = sinkpad, "Got no UTC time before EOS");
                    return Err(gst::FlowError::Error);
                } else {
                    return Err(gst_base::AGGREGATOR_FLOW_NEED_DATA);
                }
            };
            Self::check_buffer(
                &buffer,
                sinkpad,
                stream.delta_frames,
                stream.discard_header_buffers,
            )?;

            let segment = match sinkpad.segment().downcast::<gst::ClockTime>().ok() {
                Some(segment) => segment,
                None => {
                    gst::error!(CAT, obj = sinkpad, "Got buffer before segment");
                    return Err(gst::FlowError::Error);
                }
            };

            let utc_time = match get_utc_time_from_buffer(&buffer) {
                Some(utc_time) => utc_time,
                None => {
                    pre_queue.push_back((segment, buffer));
                    return Err(gst_base::AGGREGATOR_FLOW_NEED_DATA);
                }
            };

            let running_time = segment.to_running_time_full(buffer.pts().unwrap()).unwrap();
            gst::info!(
                CAT,
                obj = sinkpad,
                "Got initial UTC time {utc_time} at PTS running time {running_time}",
            );

            let mapping = (running_time, utc_time);
            stream.running_time_utc_time_mapping = Some(mapping);

            // Push the buffer onto the pre-queue and re-timestamp it and all other buffers
            // based on the mapping above.
            pre_queue.push_back((segment, buffer));

            for (segment, buffer) in pre_queue.iter_mut() {
                let buffer = buffer.make_mut();

                let pts = segment.to_running_time_full(buffer.pts().unwrap()).unwrap();
                let pts_utc_time = running_time_to_utc_time(pts, mapping).ok_or_else(|| {
                    gst::error!(CAT, obj = sinkpad, "Stream has negative PTS UTC time");
                    gst::FlowError::Error
                })?;
                gst::trace!(
                    CAT,
                    obj = sinkpad,
                    "Mapped PTS running time {pts} to UTC time {pts_utc_time}"
                );
                buffer.set_pts(pts_utc_time);

                if let Some(dts) = buffer.dts() {
                    let dts = segment.to_running_time_full(dts).unwrap();
                    let dts_utc_time = running_time_to_utc_time(dts, mapping).ok_or_else(|| {
                        gst::error!(CAT, obj = sinkpad, "Stream has negative DTS UTC time");
                        gst::FlowError::Error
                    })?;
                    gst::trace!(
                        CAT,
                        obj = sinkpad,
                        "Mapped DTS running time {dts} to UTC time {dts_utc_time}"
                    );
                    buffer.set_dts(dts_utc_time);
                }

                *segment = gst::FormattedSegment::default();
            }

            // Fall through below and pop the first buffer finally
        }

        if let Some((segment, buffer)) = stream.pre_queue.pop_front() {
            return Ok(Some((segment, buffer)));
        }

        // If the mapping is set, then we would get the buffer always from the pre-queue:
        // - either it was set before already, in which case the next buffer would've been peeked
        //   for calculating the duration to the previous buffer, and then put into the pre-queue
        // - or this is the very first buffer and we just put it into the queue overselves above
        if self.obj().class().as_ref().variant == Variant::ONVIF {
            if stream.sinkpad.is_eos() {
                return Ok(None);
            }
            unreachable!();
        }

        let Some(buffer) = stream.sinkpad.pop_buffer() else {
            return Ok(None);
        };
        Self::check_buffer(
            &buffer,
            &stream.sinkpad,
            stream.delta_frames,
            stream.discard_header_buffers,
        )?;

        let segment = match stream.sinkpad.segment().downcast::<gst::ClockTime>().ok() {
            Some(segment) => segment,
            None => {
                gst::error!(CAT, obj = stream.sinkpad, "Got buffer before segment");
                return Err(gst::FlowError::Error);
            }
        };

        Ok(Some((segment, buffer)))
    }

    /// Queue a buffer and calculate its duration.
    ///
    /// Returns `Ok(())` if a buffer with duration is known or if the stream is EOS and a buffer is
    /// queued, i.e. if this stream is ready to be processed.
    ///
    /// Returns `Err(Eos)` if nothing is queued and the stream is EOS.
    ///
    /// Returns `Err(AGGREGATOR_FLOW_NEED_DATA)` if more data is needed.
    ///
    /// Returns `Err(Error)` on errors.
    fn queue_buffer(&self, stream: &mut Stream) -> Result<(), gst::FlowError> {
        // Loop up to two times here to first retrieve the current buffer and then potentially
        // already calculate its duration based on the next queued buffer.
        loop {
            match stream.pending_buffer {
                Some(PendingBuffer {
                    duration: Some(_), ..
                }) => return Ok(()),
                Some(PendingBuffer { ref buffer, .. })
                    if stream.discard_header_buffers
                        && buffer.flags().contains(gst::BufferFlags::HEADER) =>
                {
                    return Err(gst_base::AGGREGATOR_FLOW_NEED_DATA);
                }
                Some(PendingBuffer {
                    timestamp,
                    pts,
                    ref buffer,
                    ref mut duration,
                    ..
                }) => {
                    let peek_outcome = self.peek_buffer(
                        &stream.sinkpad,
                        stream.delta_frames,
                        stream.discard_header_buffers,
                        &mut stream.pre_queue,
                        &stream.running_time_utc_time_mapping,
                    )?;
                    // Already have a pending buffer but no duration, so try to get that now
                    let (segment, buffer) = match peek_outcome {
                        Some(res) => res,
                        None => {
                            if stream.sinkpad.is_eos() {
                                let dur = buffer.duration().unwrap_or(gst::ClockTime::ZERO);
                                gst::trace!(
                                    CAT,
                                    obj = stream.sinkpad,
                                    "Stream is EOS, using {dur} as duration for queued buffer",
                                );

                                let pts = pts + dur;
                                if stream.end_pts.is_none_or(|end_pts| end_pts < pts) {
                                    gst::trace!(CAT, obj = stream.sinkpad, "Stream end PTS {pts}");
                                    stream.end_pts = Some(pts);
                                }

                                *duration = Some(dur);

                                return Ok(());
                            } else {
                                gst::trace!(
                                    CAT,
                                    obj = stream.sinkpad,
                                    "Stream has no buffer queued"
                                );
                                return Err(gst_base::AGGREGATOR_FLOW_NEED_DATA);
                            }
                        }
                    };

                    // Was checked above
                    let pts_position = buffer.pts().unwrap();
                    let next_timestamp_position = if stream.delta_frames.requires_dts() {
                        // Was checked above
                        buffer.dts().unwrap()
                    } else {
                        pts_position
                    };

                    let next_timestamp = segment
                        .to_running_time_full(next_timestamp_position)
                        .unwrap();

                    gst::trace!(
                        CAT,
                        obj = stream.sinkpad,
                        "Stream has buffer with timestamp {next_timestamp} queued",
                    );

                    let dur = next_timestamp
                        .saturating_sub(timestamp)
                        .positive()
                        .unwrap_or_else(|| {
                            gst::warning!(
                                CAT,
                                obj = stream.sinkpad,
                                "Stream timestamps going backwards {next_timestamp} < {timestamp}",
                            );
                            gst::ClockTime::ZERO
                        });

                    gst::trace!(
                        CAT,
                        obj = stream.sinkpad,
                        "Using {dur} as duration for queued buffer",
                    );

                    let pts = pts + dur;
                    if stream.end_pts.is_none_or(|end_pts| end_pts < pts) {
                        gst::trace!(CAT, obj = stream.sinkpad, "Stream end PTS {pts}");
                        stream.end_pts = Some(pts);
                    }

                    *duration = Some(dur);

                    // If the stream is AV1, we need  to parse the SequenceHeader OBU to include in the
                    // extra data of the 'av1C' box. It makes the stream playable in some browsers.
                    let s = stream.caps.structure(0).unwrap();
                    if !buffer.flags().contains(gst::BufferFlags::DELTA_UNIT)
                        && s.name().as_str() == "video/x-av1"
                    {
                        let buf_map = buffer.map_readable().map_err(|_| {
                            gst::error!(CAT, obj = stream.sinkpad, "Failed to map buffer");
                            gst::FlowError::Error
                        })?;
                        stream.extra_header_data = read_seq_header_obu_bytes(buf_map.as_slice())
                            .map_err(|_| {
                                gst::error!(
                                    CAT,
                                    obj = stream.sinkpad,
                                    "Failed to parse AV1 SequenceHeader OBU"
                                );
                                gst::FlowError::Error
                            })?;
                    }

                    return Ok(());
                }
                None => {
                    // Have no buffer queued at all yet

                    let (segment, buffer) = match self.pop_buffer(stream)? {
                        Some(res) => res,
                        None => {
                            if stream.sinkpad.is_eos() {
                                gst::trace!(CAT, obj = stream.sinkpad, "Stream is EOS",);

                                return Err(gst::FlowError::Eos);
                            } else {
                                gst::trace!(
                                    CAT,
                                    obj = stream.sinkpad,
                                    "Stream has no buffer queued"
                                );
                                return Err(gst_base::AGGREGATOR_FLOW_NEED_DATA);
                            }
                        }
                    };

                    // Was checked above
                    let pts_position = buffer.pts().unwrap();
                    let dts_position = buffer.dts();

                    let pts = segment
                        .to_running_time_full(pts_position)
                        .unwrap()
                        .positive()
                        .unwrap_or_else(|| {
                            gst::error!(
                                CAT,
                                obj = stream.sinkpad,
                                "Stream has negative PTS running time"
                            );
                            gst::ClockTime::ZERO
                        });

                    let dts = dts_position
                        .map(|dts_position| segment.to_running_time_full(dts_position).unwrap());

                    let timestamp = if stream.delta_frames.requires_dts() {
                        // Was checked above
                        let dts = dts.unwrap();

                        if stream.start_dts.is_none() {
                            gst::debug!(CAT, obj = stream.sinkpad, "Stream start DTS {dts}");
                            stream.start_dts = Some(dts);
                        }

                        dts
                    } else {
                        gst::Signed::Positive(pts)
                    };

                    if stream
                        .earliest_pts
                        .is_none_or(|earliest_pts| earliest_pts > pts)
                    {
                        gst::debug!(CAT, obj = stream.sinkpad, "Stream earliest PTS {pts}");
                        stream.earliest_pts = Some(pts);
                    }

                    let composition_time_offset = if stream.delta_frames.requires_dts() {
                        let pts = gst::Signed::Positive(pts);
                        let dts = dts.unwrap(); // set above

                        Some(i64::try_from((pts - dts).nseconds()).map_err(|_| {
                            gst::error!(CAT, obj = stream.sinkpad, "Too big PTS/DTS difference");
                            gst::FlowError::Error
                        })?)
                    } else {
                        None
                    };

                    gst::trace!(
                        CAT,
                        obj = stream.sinkpad,
                        "Stream has buffer of size {} with timestamp {timestamp} pending",
                        buffer.size(),
                    );

                    stream.pending_buffer = Some(PendingBuffer {
                        buffer,
                        timestamp,
                        pts,
                        composition_time_offset,
                        duration: None,
                    });
                }
            }
        }
    }

    fn find_earliest_stream(
        &self,
        settings: &Settings,
        state: &mut State,
        buffers: &mut gst::BufferListRef,
    ) -> Result<Option<usize>, gst::FlowError> {
        if let Some(current_stream_idx) = state.current_stream_idx {
            // If a stream was previously selected, check if another buffer from
            // this stream can be consumed or if that would exceed the interleave.

            let single_stream = state.streams.len() == 1;
            let stream = &mut state.streams[current_stream_idx];

            match self.queue_buffer(stream) {
                Ok(_) => {
                    assert!(matches!(
                        stream.pending_buffer,
                        Some(PendingBuffer {
                            duration: Some(_),
                            ..
                        })
                    ));

                    if single_stream
                        || (settings.interleave_bytes.is_none_or(|interleave_bytes| {
                            interleave_bytes >= stream.queued_chunk_bytes
                        }) && settings.interleave_time.is_none_or(|interleave_time| {
                            interleave_time >= stream.queued_chunk_time
                        }))
                    {
                        gst::trace!(CAT,
                            obj = stream.sinkpad,
                            "Continuing current chunk: single stream {single_stream}, or {} >= {} and {} >= {}",
                            gst::format::Bytes::from_u64(stream.queued_chunk_bytes),
                            settings.interleave_bytes.map(gst::format::Bytes::from_u64).display(),
                            stream.queued_chunk_time, settings.interleave_time.display(),
                        );
                        return Ok(Some(current_stream_idx));
                    }
                    let num_bytes_added =
                        self.flush_aux_info(buffers, stream, state.current_offset);
                    state.current_offset += num_bytes_added;
                    state.mdat_size += num_bytes_added;

                    state.current_stream_idx = None;
                    gst::debug!(
                        CAT,
                        obj = stream.sinkpad,
                        "Switching to next chunk: {} < {} and {} < {}",
                        gst::format::Bytes::from_u64(stream.queued_chunk_bytes),
                        settings
                            .interleave_bytes
                            .map(gst::format::Bytes::from_u64)
                            .display(),
                        stream.queued_chunk_time,
                        settings.interleave_time.display(),
                    );
                }
                Err(gst::FlowError::Eos) => {
                    gst::debug!(
                        CAT,
                        obj = stream.sinkpad,
                        "Stream is EOS, switching to next stream"
                    );
                    let num_bytes_added =
                        self.flush_aux_info(buffers, stream, state.current_offset);
                    state.current_offset += num_bytes_added;
                    state.mdat_size += num_bytes_added;
                    state.current_stream_idx = None;
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }

        // Otherwise find the next earliest stream here
        let mut earliest_stream = None;
        let mut all_have_data_or_eos = true;
        let mut all_eos = true;

        for (idx, stream) in state.streams.iter_mut().enumerate() {
            // First queue a buffer on each stream and try to get the duration

            match self.queue_buffer(stream) {
                Ok(_) => {
                    assert!(matches!(
                        stream.pending_buffer,
                        Some(PendingBuffer {
                            duration: Some(_),
                            ..
                        })
                    ));

                    let timestamp = stream.pending_buffer.as_ref().unwrap().timestamp;

                    gst::trace!(CAT, obj = stream.sinkpad, "Stream at timestamp {timestamp}",);

                    all_eos = false;

                    if earliest_stream
                        .as_ref()
                        .is_none_or(|(_idx, _stream, earliest_timestamp)| {
                            *earliest_timestamp > timestamp
                        })
                    {
                        earliest_stream = Some((idx, stream, timestamp));
                    }
                }
                Err(gst::FlowError::Eos) => {
                    all_eos &= true;
                    continue;
                }
                Err(gst_base::AGGREGATOR_FLOW_NEED_DATA) => {
                    all_have_data_or_eos = false;
                    continue;
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }

        if !all_have_data_or_eos {
            gst::trace!(CAT, imp = self, "Not all streams have a buffer or are EOS");
            Err(gst_base::AGGREGATOR_FLOW_NEED_DATA)
        } else if all_eos {
            gst::info!(CAT, imp = self, "All streams are EOS");
            Err(gst::FlowError::Eos)
        } else if let Some((idx, stream, earliest_timestamp)) = earliest_stream {
            gst::debug!(
                CAT,
                obj = stream.sinkpad,
                "Stream is earliest stream with timestamp {earliest_timestamp}",
            );

            gst::debug!(
                CAT,
                obj = stream.sinkpad,
                "Starting new chunk at offset {}",
                state.current_offset,
            );

            stream.chunks.push(Chunk {
                offset: state.current_offset,
                samples: Vec::new(),
            });
            stream.queued_chunk_time = gst::ClockTime::ZERO;
            stream.queued_chunk_bytes = 0;

            state.current_stream_idx = Some(idx);
            Ok(Some(idx))
        } else {
            unreachable!()
        }
    }

    fn flush_aux_info(
        &self,
        buffers: &mut gst::BufferListRef,
        stream: &mut Stream,
        initial_offset: u64,
    ) -> u64 {
        gst::debug!(
            CAT,
            obj = stream.sinkpad,
            "Flushing {} pending entries of auxiliary information from stream {} to mdat at end of current chunk",
            stream.pending_aux_info_data.len(),
            stream.sinkpad.name(),
        );
        let mut num_bytes_added = 0u64;
        while let Some(pending_aux_info) = stream.pending_aux_info_data.pop_front() {
            // We don't handle the case where the aux_info_type is None - no idea what the semantics of that would be
            assert!(pending_aux_info.aux_info_type.is_some());
            let maybe_index = stream.aux_info.iter().position(|aux_info| {
                (aux_info.aux_info_type == pending_aux_info.aux_info_type)
                    && (aux_info.aux_info_type_parameter
                        == pending_aux_info.aux_info_type_parameter)
            });
            let index: usize = match maybe_index {
                Some(index) => index,
                None => {
                    // We did not already have a matching entry in the stream, add it
                    let aux_info = AuxiliaryInformation {
                        aux_info_type: pending_aux_info.aux_info_type,
                        aux_info_type_parameter: pending_aux_info.aux_info_type_parameter,
                        entries: Vec::new(),
                    };
                    stream.aux_info.push(aux_info);
                    stream.aux_info.len() - 1
                }
            };
            let pending_aux_info_data = gst::Buffer::from_slice(pending_aux_info.data);
            assert!(pending_aux_info_data.size() <= u8::MAX as usize);
            let entry_size = pending_aux_info_data.size() as u8;
            buffers.add(pending_aux_info_data);
            stream.aux_info[index]
                .entries
                .push(AuxiliaryInformationEntry {
                    entry_offset: initial_offset + num_bytes_added,
                    entry_len: entry_size,
                });
            num_bytes_added += entry_size as u64;
        }
        num_bytes_added
    }

    fn drain_buffers(
        &self,
        settings: &Settings,
        state: &mut State,
        buffers: &mut gst::BufferListRef,
    ) -> Result<(), gst::FlowError> {
        // Now we can start handling buffers
        while let Some(idx) = self.find_earliest_stream(settings, state, buffers)? {
            let stream = &mut state.streams[idx];
            let buffer = stream.pending_buffer.take().unwrap();

            if buffer.buffer.flags().contains(gst::BufferFlags::GAP)
                && buffer.buffer.flags().contains(gst::BufferFlags::DROPPABLE)
                && buffer.buffer.size() == 0
            {
                gst::trace!(CAT, obj = stream.sinkpad, "Skipping gap buffer {buffer:?}");

                // If a new chunk was just started for the gap buffer, don't bother and get rid
                // of this chunk again for now and search for the next stream.
                if let Some(chunk) = stream.chunks.last() {
                    if chunk.samples.is_empty() {
                        let _ = stream.chunks.pop();
                        state.current_stream_idx = None;
                    } else {
                        // Add duration of the gap to the current chunk
                        stream.queued_chunk_time += buffer.duration.unwrap();
                    }
                }

                // Add the duration of the gap buffer to the previously written out sample.
                if let Some(previous_sample) =
                    stream.chunks.last_mut().and_then(|c| c.samples.last_mut())
                {
                    gst::trace!(
                        CAT,
                        obj = stream.sinkpad,
                        "Adding gap duration {} to previous sample",
                        buffer.duration.unwrap()
                    );
                    previous_sample.duration += buffer.duration.unwrap();
                } else {
                    gst::trace!(
                        CAT,
                        obj = stream.sinkpad,
                        "Resetting stream start time because it started with a gap"
                    );
                    // If there was no previous sample yet then the next sample needs to start
                    // earlier or alternatively we change the start PTS. We do the latter here
                    // as otherwise the first sample would be displayed too early.
                    stream.earliest_pts = None;
                    stream.start_dts = None;
                    stream.end_pts = None;
                }

                continue;
            }

            gst::trace!(
                CAT,
                obj = stream.sinkpad,
                "Handling buffer {buffer:?} at offset {}",
                state.current_offset
            );

            let duration = buffer.duration.unwrap();
            let composition_time_offset = buffer.composition_time_offset;

            if let Err(err) = self.add_elst_info(&buffer, stream) {
                gst::error!(CAT, "Failed to add elst info: {:#}", err);
            }

            let mut buffer = buffer.buffer;
            #[cfg(feature = "v1_28")]
            {
                if settings.with_precision_timestamps {
                    // Builds a TAITimestampPacket structure as defined in ISO/IEC 23001-17 Amendment 1, Section 8.1.2
                    // That will be written out as `stai` aux info per Section 8.1.3.
                    // See ISO/IEC 14496-12 Section 8.7.8 and 8.7.9 for more on aux info.
                    if let Some(meta) = buffer
                        .iter_meta::<gst::ReferenceTimestampMeta>()
                        .find(|m| m.reference().can_intersect(&TAI1958_CAPS) && m.info().is_some())
                    {
                        gst::trace!(
                            CAT,
                            imp = self,
                            "got TAI ReferenceTimestampMeta on the buffer"
                        );
                        let mut timestamp_packet = Vec::<u8>::with_capacity(9);
                        timestamp_packet.extend(meta.timestamp().nseconds().to_be_bytes());
                        state.last_tai_timestamp = meta.timestamp().nseconds();
                        let iso23001_17_timestamp_info = meta.info().unwrap(); // checked in filter
                        let mut timestamp_packet_flags = 0u8;
                        if let Ok(synced) =
                            iso23001_17_timestamp_info.get::<bool>("synchronization-state")
                        {
                            gst::trace!(
                                CAT,
                                imp = self,
                                "synchronized to atomic source: {:?}",
                                synced
                            );
                            if synced {
                                timestamp_packet_flags |= 0x80u8;
                            }
                        } else {
                            gst::info!(CAT, imp=self, "TAI ReferenceTimestampMeta did not contain expected synchronisation state, assuming not synchronised");
                        }
                        if let Ok(generation_failure) =
                            iso23001_17_timestamp_info.get::<bool>("timestamp-generation-failure")
                        {
                            gst::trace!(
                                CAT,
                                imp = self,
                                "timestamp generation failure: {:?}",
                                generation_failure
                            );
                            if generation_failure {
                                timestamp_packet_flags |= 0x40u8;
                            }
                        } else if meta.timestamp().nseconds() > state.last_tai_timestamp {
                            gst::info!(CAT, imp=self, "TAI ReferenceTimestampMeta did not contain expected generation failure flag, timestamp looks OK, assuming OK");
                        } else {
                            gst::warning!(CAT, imp=self, "TAI ReferenceTimestampMeta did not contain expected generation failure flag and unexpected timestamp value, assuming generation failure");
                            timestamp_packet_flags |= 0x40u8;
                        }
                        if let Ok(timestamp_is_modified) =
                            iso23001_17_timestamp_info.get::<bool>("timestamp-is-modified")
                        {
                            gst::trace!(
                                CAT,
                                imp = self,
                                "timestamp is modified: {:?}",
                                timestamp_is_modified
                            );
                            if timestamp_is_modified {
                                timestamp_packet_flags |= 0x20u8;
                            }
                        } else {
                            gst::info!(CAT, imp=self, "TAI ReferenceTimestampMeta did not contain expected modification state value, assuming not modified");
                        }
                        timestamp_packet.extend(timestamp_packet_flags.to_be_bytes());
                        stream.pending_aux_info_data.push_back(PendingAuxInfoEntry {
                            aux_info_type: Some(*b"stai"),
                            aux_info_type_parameter: 0,
                            data: timestamp_packet,
                        });
                    } else {
                        // generate a failure packet, because we always need aux info for a sample
                        let mut timestamp_packet = Vec::<u8>::with_capacity(9);
                        // The timestamp must monotonically increase
                        let timestamp = state.last_tai_timestamp + 1;
                        timestamp_packet.extend(timestamp.to_be_bytes());
                        state.last_tai_timestamp = timestamp;
                        let flags = 0x40u8; // not sync'd | generation failure | not modified,
                        timestamp_packet.extend(flags.to_be_bytes());
                        stream.pending_aux_info_data.push_back(PendingAuxInfoEntry {
                            aux_info_type: Some(*b"stai"),
                            aux_info_type_parameter: 0,
                            data: timestamp_packet,
                        });
                    }
                }
            }
            stream.queued_chunk_time += duration;
            stream.queued_chunk_bytes += buffer.size() as u64;

            stream.chunks.last_mut().unwrap().samples.push(Sample {
                sync_point: !buffer.flags().contains(gst::BufferFlags::DELTA_UNIT),
                duration,
                composition_time_offset,
                size: buffer.size() as u32,
            });

            {
                let buffer = buffer.make_mut();
                buffer.set_dts(None);
                buffer.set_pts(None);
                buffer.set_duration(duration);
                buffer.unset_flags(gst::BufferFlags::all());
            }

            state.current_offset += buffer.size() as u64;
            state.mdat_size += buffer.size() as u64;
            buffers.add(buffer);
        }

        Ok(())
    }

    fn create_streams(&self, state: &mut State) -> Result<(), gst::FlowError> {
        gst::info!(CAT, imp = self, "Creating streams");

        for pad in self
            .obj()
            .sink_pads()
            .into_iter()
            .map(|pad| pad.downcast::<crate::isobmff::MP4MuxPad>().unwrap())
        {
            // Check if language or orientation tags have already been
            // received
            let mut stream_orientation = Default::default();
            let mut global_orientation = Default::default();
            let mut language_code = None;
            let mut avg_bitrate = None;
            let mut max_bitrate = None;
            let mut tai_clock_info = None;
            let mut clock_type = TaicClockType::Unknown;
            let mut found_taic_part = false;
            let mut time_uncertainty = TAIC_TIME_UNCERTAINTY_UNKNOWN;
            pad.sticky_events_foreach(|ev| {
                if let gst::EventView::Tag(ev) = ev.view() {
                    let tag = ev.tag();
                    if let Some(lang) = tag.get::<gst::tags::LanguageCode>() {
                        let lang = lang.get();
                        gst::trace!(
                            CAT,
                            obj = pad,
                            "Received language code from tags: {:?}",
                            lang
                        );

                        // There is no header field for global
                        // language code, maybe because it does not
                        // really make sense, global language tags are
                        // considered to be stream local
                        if tag.scope() == gst::TagScope::Global {
                            gst::info!(
                                CAT,
                                obj = pad,
                                "Language tags scoped 'global' are considered stream tags",
                            );
                        }
                        language_code = Stream::parse_language_code(lang);
                    }
                    if let Some(orientation) = tag.get::<gst::tags::ImageOrientation>() {
                        gst::trace!(
                            CAT,
                            obj = pad,
                            "Received image orientation from tags: {:?}",
                            orientation.get(),
                        );

                        if tag.scope() == gst::TagScope::Global {
                            global_orientation = TransformMatrix::from_tag(self, ev);
                        } else {
                            stream_orientation = Some(TransformMatrix::from_tag(self, ev));
                        }
                    }
                    if let Some(bitrate) = tag
                        .get::<gst::tags::MaximumBitrate>()
                        .filter(|br| br.get() > 0 && br.get() < u32::MAX)
                    {
                        let bitrate = bitrate.get();
                        gst::trace!(
                            CAT,
                            obj = pad,
                            "Received maximum bitrate from tags: {:?}",
                            bitrate
                        );

                        if tag.scope() == gst::TagScope::Global {
                            gst::info!(
                                CAT,
                                obj = pad,
                                "Bitrate tags scoped 'global' are considered stream tags",
                            );
                        }
                        max_bitrate = Some(bitrate);
                    }
                    if let Some(bitrate) = tag
                        .get::<gst::tags::Bitrate>()
                        .filter(|br| br.get() > 0 && br.get() < u32::MAX)
                    {
                        let bitrate = bitrate.get();
                        gst::trace!(CAT, obj = pad, "Received bitrate from tags: {:?}", bitrate);

                        if tag.scope() == gst::TagScope::Global {
                            gst::info!(
                                CAT,
                                obj = pad,
                                "Bitrate tags scoped 'global' are considered stream tags",
                            );
                        }
                        avg_bitrate = Some(bitrate);
                    }
                    if let Some(tag_value) = ev.tag().get::<crate::isobmff::PrecisionClockTypeTag>() {
                        let clock_type_str = tag_value.get();
                        gst::debug!(
                            CAT,
                            imp = self,
                            "Received TAI clock type from tags: {:?}",
                            clock_type_str
                        );
                        clock_type = match clock_type_str.parse() {
                            Ok(t) => {
                                found_taic_part = true;
                                t
                            }
                            Err(err) => {
                                gst::warning!(CAT, imp = self, "error parsing TAIClockType tag value: {}", err);
                                TaicClockType::Unknown
                            }
                        };
                        if tag.scope() == gst::TagScope::Global {
                            gst::info!(
                                CAT,
                                obj = pad,
                                "TAI clock tags scoped 'global' are treated as if they were stream tags.",
                            );
                        }
                    }
                    if let Some(tag_value) = ev
                        .tag()
                        .get::<crate::isobmff::PrecisionClockTimeUncertaintyNanosecondsTag>()
                    {
                        let time_uncertainty_from_tag = tag_value.get();
                        if time_uncertainty_from_tag < 1 {
                            gst::warning!(
                                CAT,
                                imp = self,
                                "Ignoring non-positive TAI clock uncertainty from tags: {:?}",
                                time_uncertainty_from_tag
                            );
                        } else {
                            time_uncertainty = time_uncertainty_from_tag as u64;
                            gst::debug!(
                                CAT,
                                imp = self,
                                "Received TAI clock uncertainty from tags: {:?}",
                                time_uncertainty
                            );
                            if tag.scope() == gst::TagScope::Global {
                                gst::info!(
                                    CAT,
                                    obj = pad,
                                    "TAI clock tags scoped 'global' are treated as if they were stream tags.",
                                );
                            }
                            found_taic_part = true;
                        }
                    }
                }
                std::ops::ControlFlow::Continue(gst::EventForeachAction::Keep)
            });

            let caps = match pad.current_caps() {
                Some(caps) => caps,
                None => {
                    gst::warning!(CAT, obj = pad, "Skipping pad without caps");
                    continue;
                }
            };

            // TODO: also set this when we have GIMI implemented
            if found_taic_part {
                // TODO: set remaining parts if there are tags implemented
                tai_clock_info = Some(TaiClockInfo::new(clock_type, time_uncertainty));
            }

            gst::info!(CAT, obj = pad, "Configuring caps {caps:?}");

            let s = caps.structure(0).unwrap();

            let mut delta_frames = DeltaFrames::IntraOnly;
            let mut discard_header_buffers = false;
            let mut codec_specific_boxes = Vec::new();
            match s.name().as_str() {
                "video/x-h264" | "video/x-h265" => {
                    if !s.has_field_with_type("codec_data", gst::Buffer::static_type()) {
                        gst::error!(CAT, obj = pad, "Received caps without codec_data");
                        return Err(gst::FlowError::NotNegotiated);
                    }
                    delta_frames = DeltaFrames::Bidirectional;
                }
                "video/x-vp8" => {
                    delta_frames = DeltaFrames::PredictiveOnly;
                }
                "video/x-vp9" => {
                    if !s.has_field_with_type("colorimetry", str::static_type()) {
                        gst::error!(CAT, obj = pad, "Received caps without colorimetry");
                        return Err(gst::FlowError::NotNegotiated);
                    }
                    delta_frames = DeltaFrames::PredictiveOnly;
                }
                "video/x-av1" => {
                    delta_frames = DeltaFrames::PredictiveOnly;
                }
                "image/jpeg" => (),
                "video/x-raw" => (),
                "audio/mpeg" => {
                    if !s.has_field_with_type("codec_data", gst::Buffer::static_type()) {
                        gst::error!(CAT, obj = pad, "Received caps without codec_data");
                        return Err(gst::FlowError::NotNegotiated);
                    }
                }
                "audio/x-opus" => {
                    if let Some(header) = s
                        .get::<gst::ArrayRef>("streamheader")
                        .ok()
                        .and_then(|a| a.first().and_then(|v| v.get::<gst::Buffer>().ok()))
                    {
                        if gst_pbutils::codec_utils_opus_parse_header(&header, None).is_err() {
                            gst::error!(CAT, obj = pad, "Received invalid Opus header");
                            return Err(gst::FlowError::NotNegotiated);
                        }
                    } else if gst_pbutils::codec_utils_opus_parse_caps(&caps, None).is_err() {
                        gst::error!(CAT, obj = pad, "Received invalid Opus caps");
                        return Err(gst::FlowError::NotNegotiated);
                    }
                }
                "audio/x-flac" => {
                    discard_header_buffers = true;
                    if let Err(e) = s.get::<gst::ArrayRef>("streamheader") {
                        gst::error!(
                            CAT,
                            obj = pad,
                            "Muxing FLAC into MP4 needs streamheader: {}",
                            e
                        );
                        return Err(gst::FlowError::NotNegotiated);
                    };
                }
                "audio/x-ac3" | "audio/x-eac3" => {
                    let Some(first_buffer) = pad.peek_buffer() else {
                        gst::error!(
                            CAT,
                            obj = pad,
                            "Need first buffer for AC-3 / EAC-3 when creating header"
                        );
                        return Err(gst::FlowError::NotNegotiated);
                    };
                    match s.name().as_str() {
                        "audio/x-ac3" => {
                            codec_specific_boxes = match create_dac3(&first_buffer) {
                                Ok(boxes) => boxes,
                                Err(err) => {
                                    gst::error!(
                                        CAT,
                                        obj = pad,
                                        "Failed to create AC-3 codec specific box: {err}"
                                    );
                                    return Err(gst::FlowError::NotNegotiated);
                                }
                            };
                        }
                        "audio/x-eac3" => {
                            codec_specific_boxes = match create_dec3(&first_buffer) {
                                Ok(boxes) => boxes,
                                Err(err) => {
                                    gst::error!(
                                        CAT,
                                        obj = pad,
                                        "Failed to create EAC-3 codec specific box: {err}"
                                    );
                                    return Err(gst::FlowError::NotNegotiated);
                                }
                            };
                        }
                        _ => unreachable!(),
                    }
                }
                "audio/x-alaw" | "audio/x-mulaw" => (),
                "audio/x-adpcm" => (),
                "application/x-onvif-metadata" => (),
                _ => unreachable!(),
            }

            state.streams.push(Stream {
                sinkpad: pad,
                pre_queue: VecDeque::new(),
                caps,
                delta_frames,
                discard_header_buffers,
                chunks: Vec::new(),
                pending_buffer: None,
                queued_chunk_time: gst::ClockTime::ZERO,
                queued_chunk_bytes: 0,
                start_dts: None,
                earliest_pts: None,
                elst_infos: Default::default(),
                end_pts: None,
                running_time_utc_time_mapping: None,
                extra_header_data: None,
                codec_specific_boxes,
                language_code,
                global_orientation,
                stream_orientation,
                max_bitrate,
                avg_bitrate,
                tai_clock_info,
                aux_info: Vec::new(),
                pending_aux_info_data: VecDeque::new(),
            });
        }

        if state.streams.is_empty() {
            gst::error!(CAT, imp = self, "No streams available");
            return Err(gst::FlowError::Error);
        }

        // Sort video streams first and then audio streams and then metadata streams, and each group by pad name.
        state.streams.sort_by(|a, b| {
            let order_of_caps = |caps: &gst::CapsRef| {
                let s = caps.structure(0).unwrap();

                if s.name().starts_with("video/") {
                    0
                } else if s.name().starts_with("audio/") {
                    1
                } else if s.name().starts_with("application/x-onvif-metadata") {
                    2
                } else {
                    unimplemented!();
                }
            };

            let st_a = order_of_caps(&a.caps);
            let st_b = order_of_caps(&b.caps);

            if st_a == st_b {
                return a.sinkpad.name().cmp(&b.sinkpad.name());
            }

            st_a.cmp(&st_b)
        });

        Ok(())
    }
}

#[glib::object_subclass]
impl ObjectSubclass for MP4Mux {
    const NAME: &'static str = "GstRsMP4Mux";
    type Type = crate::isobmff::MP4Mux;
    type ParentType = gst_base::Aggregator;
    type Class = Class;
    type Interfaces = (gst::ChildProxy,);
}

impl ObjectImpl for MP4Mux {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt64::builder("interleave-bytes")
                    .nick("Interleave Bytes")
                    .blurb("Interleave between streams in bytes")
                    .default_value(DEFAULT_INTERLEAVE_BYTES.unwrap_or(0))
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt64::builder("interleave-time")
                    .nick("Interleave Time")
                    .blurb("Interleave between streams in nanoseconds")
                    .default_value(
                        DEFAULT_INTERLEAVE_TIME
                            .map(gst::ClockTime::nseconds)
                            .unwrap_or(u64::MAX),
                    )
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("movie-timescale")
                    .nick("Movie Timescale")
                    .blurb("Timescale to use for the movie (units per second, 0 is automatic)")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecString::builder("extra-brands")
                    .nick("Extra Brands")
                    .blurb("Comma-separated list of 4-character brand codes (e.g. duke,sook)")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("tai-precision-timestamps")
                    .nick("Precision Timestamps")
                    .blurb("Whether to encode ISO/IEC 23001-17 TAI timestamps as auxiliary data")
                    .default_value(false)
                    .mutable_ready()
                    .build(),
            ]
        });

        &PROPERTIES
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "interleave-bytes" => {
                let mut settings = self.settings.lock().unwrap();
                settings.interleave_bytes = match value.get().expect("type checked upstream") {
                    0 => None,
                    v => Some(v),
                };
            }

            "interleave-time" => {
                let mut settings = self.settings.lock().unwrap();
                settings.interleave_time = match value.get().expect("type checked upstream") {
                    Some(gst::ClockTime::ZERO) | None => None,
                    v => v,
                };
            }

            "movie-timescale" => {
                let mut settings = self.settings.lock().unwrap();
                settings.movie_timescale = value.get().expect("type checked upstream");
            }

            "extra-brands" => {
                let mut settings = self.settings.lock().unwrap();

                if let Some(input) = value.get::<Option<String>>().ok().flatten() {
                    settings.extra_brands.clear();

                    for token in input.split(',') {
                        let trimmed = token.trim();

                        if trimmed.len() != 4 {
                            gst::error!(
                                CAT,
                                imp = self,
                                "Skipping invalid brand (must be 4 chars): {trimmed}"
                            );
                            continue;
                        }

                        // Convert to 4-byte array
                        let bytes = trimmed.as_bytes();
                        let brand = [bytes[0], bytes[1], bytes[2], bytes[3]];
                        settings.extra_brands.push(brand);
                    }
                }
            }

            "tai-precision-timestamps" => {
                let mut settings = self.settings.lock().unwrap();
                settings.with_precision_timestamps = value.get().expect("type checked upstream");
            }

            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "interleave-bytes" => {
                let settings = self.settings.lock().unwrap();
                settings.interleave_bytes.unwrap_or(0).to_value()
            }

            "interleave-time" => {
                let settings = self.settings.lock().unwrap();
                settings.interleave_time.to_value()
            }

            "movie-timescale" => {
                let settings = self.settings.lock().unwrap();
                settings.movie_timescale.to_value()
            }

            "extra-brands" => {
                let settings = self.settings.lock().unwrap();
                let brands_str = settings
                    .extra_brands
                    .iter()
                    .map(|fourcc| std::str::from_utf8(fourcc).unwrap().to_string())
                    .collect::<Vec<_>>()
                    .join(",");

                Some(brands_str).to_value()
            }

            "tai-precision-timestamps" => {
                let settings = self.settings.lock().unwrap();
                settings.with_precision_timestamps.to_value()
            }

            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for MP4Mux {}

impl ElementImpl for MP4Mux {
    fn request_new_pad(
        &self,
        templ: &gst::PadTemplate,
        name: Option<&str>,
        caps: Option<&gst::Caps>,
    ) -> Option<gst::Pad> {
        let state = self.state.lock().unwrap();
        if !state.streams.is_empty() {
            gst::error!(
                CAT,
                imp = self,
                "Can't request new pads after stream was started"
            );
            return None;
        }
        drop(state);

        let pad = self.parent_request_new_pad(templ, name, caps);

        if let Some(ref pad) = pad {
            let element = self.obj();
            element.child_added(pad, &pad.name());
        }

        pad
    }

    fn release_pad(&self, pad: &gst::Pad) {
        let element = self.obj();
        element.child_removed(pad, &pad.name());
        self.parent_release_pad(pad);
    }
}

impl AggregatorImpl for MP4Mux {
    fn next_time(&self) -> Option<gst::ClockTime> {
        None
    }

    fn sink_query(
        &self,
        aggregator_pad: &gst_base::AggregatorPad,
        query: &mut gst::QueryRef,
    ) -> bool {
        use gst::QueryViewMut;

        gst::trace!(CAT, obj = aggregator_pad, "Handling query {query:?}");

        match query.view_mut() {
            QueryViewMut::Caps(q) => {
                let mut allowed_caps = aggregator_pad
                    .current_caps()
                    .unwrap_or_else(|| aggregator_pad.pad_template_caps());

                // Allow framerate change
                for s in allowed_caps.make_mut().iter_mut() {
                    s.remove_field("framerate");
                }

                if let Some(filter_caps) = q.filter() {
                    let res = filter_caps
                        .intersect_with_mode(&allowed_caps, gst::CapsIntersectMode::First);
                    q.set_result(&res);
                } else {
                    q.set_result(&allowed_caps);
                }

                true
            }
            _ => self.parent_sink_query(aggregator_pad, query),
        }
    }

    fn sink_event_pre_queue(
        &self,
        aggregator_pad: &gst_base::AggregatorPad,
        mut event: gst::Event,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        use gst::EventView;

        gst::trace!(CAT, obj = aggregator_pad, "Handling event {event:?}");

        match event.view() {
            EventView::Segment(ev) => {
                if ev.segment().format() != gst::Format::Time {
                    gst::warning!(
                        CAT,
                        obj = aggregator_pad,
                        "Received non-TIME segment, replacing with default TIME segment"
                    );
                    let segment = gst::FormattedSegment::<gst::ClockTime>::new();
                    event = gst::event::Segment::builder(&segment)
                        .seqnum(event.seqnum())
                        .build();
                }
                self.parent_sink_event_pre_queue(aggregator_pad, event)
            }
            _ => self.parent_sink_event_pre_queue(aggregator_pad, event),
        }
    }

    fn sink_event(&self, aggregator_pad: &gst_base::AggregatorPad, event: gst::Event) -> bool {
        use gst::EventView;

        gst::trace!(CAT, obj = aggregator_pad, "Handling event {event:?}");

        match event.view() {
            EventView::Tag(ev) => {
                let tag = ev.tag();

                if let Some(tag_value) = tag.get::<gst::tags::LanguageCode>() {
                    let lang = tag_value.get();
                    gst::trace!(
                        CAT,
                        obj = aggregator_pad,
                        "Received language code from tags: {:?}",
                        lang
                    );

                    // Language as ISO-639-2/T
                    if let Some(language_code) = Stream::parse_language_code(lang) {
                        let mut state = self.state.lock().unwrap();

                        if tag.scope() == gst::TagScope::Global {
                            gst::info!(
                                CAT,
                                obj = aggregator_pad,
                                "Language tags scoped 'global' are considered stream tags",
                            );
                        }

                        for stream in &mut state.streams {
                            if &stream.sinkpad == aggregator_pad {
                                stream.language_code = Some(language_code);
                                break;
                            }
                        }
                    }
                }
                if let Some(tag_value) = ev.tag().get::<gst::tags::ImageOrientation>() {
                    let orientation = tag_value.get();
                    gst::trace!(
                        CAT,
                        obj = aggregator_pad,
                        "Received image orientation from tags: {:?}",
                        orientation
                    );

                    let mut state = self.state.lock().unwrap();
                    for stream in &mut state.streams {
                        if &stream.sinkpad == aggregator_pad {
                            let orientation = TransformMatrix::from_tag(self, ev);
                            if tag.scope() == gst::TagScope::Stream {
                                stream.stream_orientation = Some(orientation);
                            } else {
                                stream.global_orientation = orientation;
                            }
                            break;
                        }
                    }
                }
                if let Some(bitrate) = tag
                    .get::<gst::tags::MaximumBitrate>()
                    .filter(|br| br.get() > 0 && br.get() < u32::MAX)
                {
                    let bitrate = bitrate.get();
                    gst::trace!(
                        CAT,
                        obj = aggregator_pad,
                        "Received maximum bitrate from tags: {:?}",
                        bitrate
                    );

                    if tag.scope() == gst::TagScope::Global {
                        gst::info!(
                            CAT,
                            obj = aggregator_pad,
                            "Bitrate tags scoped 'global' are considered stream tags",
                        );
                    }

                    let mut state = self.state.lock().unwrap();
                    for stream in &mut state.streams {
                        if &stream.sinkpad == aggregator_pad {
                            stream.max_bitrate = Some(bitrate);
                            break;
                        }
                    }
                }
                if let Some(bitrate) = tag
                    .get::<gst::tags::Bitrate>()
                    .filter(|br| br.get() > 0 && br.get() < u32::MAX)
                {
                    let bitrate = bitrate.get();
                    gst::trace!(
                        CAT,
                        obj = aggregator_pad,
                        "Received bitrate from tags: {:?}",
                        bitrate
                    );

                    if tag.scope() == gst::TagScope::Global {
                        gst::info!(
                            CAT,
                            obj = aggregator_pad,
                            "Bitrate tags scoped 'global' are considered stream tags",
                        );
                    }

                    let mut state = self.state.lock().unwrap();
                    for stream in &mut state.streams {
                        if &stream.sinkpad == aggregator_pad {
                            stream.avg_bitrate = Some(bitrate);
                            break;
                        }
                    }
                }

                self.parent_sink_event(aggregator_pad, event)
            }
            _ => self.parent_sink_event(aggregator_pad, event),
        }
    }

    fn src_query(&self, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        gst::trace!(CAT, imp = self, "Handling query {query:?}");

        match query.view_mut() {
            QueryViewMut::Seeking(q) => {
                // We can't really handle seeking, it would break everything
                q.set(false, gst::ClockTime::ZERO, gst::ClockTime::NONE);
                true
            }
            _ => self.parent_src_query(query),
        }
    }

    fn src_event(&self, event: gst::Event) -> bool {
        use gst::EventView;

        gst::trace!(CAT, imp = self, "Handling event {event:?}");

        match event.view() {
            EventView::Seek(_ev) => false,
            _ => self.parent_src_event(event),
        }
    }

    fn flush(&self) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst::info!(CAT, imp = self, "Flushing");

        let mut state = self.state.lock().unwrap();
        for stream in &mut state.streams {
            stream.pending_buffer = None;
            stream.pre_queue.clear();
            stream.running_time_utc_time_mapping = None;
        }
        drop(state);

        self.parent_flush()
    }

    fn stop(&self) -> Result<(), gst::ErrorMessage> {
        gst::trace!(CAT, imp = self, "Stopping");

        let _ = self.parent_stop();

        *self.state.lock().unwrap() = State::default();

        Ok(())
    }

    fn start(&self) -> Result<(), gst::ErrorMessage> {
        gst::trace!(CAT, imp = self, "Starting");

        self.parent_start()?;

        // Always output a BYTES segment
        let segment = gst::FormattedSegment::<gst::format::Bytes>::new();
        self.obj().update_segment(&segment);

        *self.state.lock().unwrap() = State::default();

        Ok(())
    }

    fn negotiate(&self) -> bool {
        true
    }

    fn aggregate(&self, _timeout: bool) -> Result<gst::FlowSuccess, gst::FlowError> {
        let settings = self.settings.lock().unwrap().clone();
        let mut state = self.state.lock().unwrap();

        let mut buffers = gst::BufferList::new();
        let mut caps = None;

        // If no streams were created yet, collect all streams now and write the mdat.
        if state.streams.is_empty() {
            // First check if downstream is seekable. If not we can't rewrite the mdat box header!
            drop(state);

            let mut q = gst::query::Seeking::new(gst::Format::Bytes);
            if self.obj().src_pad().peer_query(&mut q) {
                if !q.result().0 {
                    gst::element_imp_error!(
                        self,
                        gst::StreamError::Mux,
                        ["Downstream is not seekable"]
                    );
                    return Err(gst::FlowError::Error);
                }
            } else {
                // Can't query downstream, have to assume downstream is seekable
                gst::warning!(CAT, imp = self, "Can't query downstream for seekability");
            }

            state = self.state.lock().unwrap();
            self.create_streams(&mut state)?;

            // Create caps now to be sent before any buffers
            caps = Some(
                gst::Caps::builder("video/quicktime")
                    .field("variant", "iso")
                    .build(),
            );

            gst::info!(
                CAT,
                imp = self,
                "Creating ftyp box at offset {}",
                state.current_offset
            );

            // ... and then create the ftyp box plus mdat box header so we can start outputting
            // actual data

            let mut major_brand = *b"iso4";
            let mut minor_version = 0u32;
            let mut compatible_brands: BTreeSet<[u8; 4]> = BTreeSet::new();
            let mut have_image_sequence = false; // we'll mark true if an image sequence
            let mut have_only_image_sequence = true; // we'll mark false if video found
            for stream in state.streams.iter().as_ref() {
                let caps_structure = stream.caps.structure(0).unwrap();

                if stream.image_sequence_mode() {
                    compatible_brands.insert(*b"iso8");
                    compatible_brands.insert(*b"unif");
                    compatible_brands.insert(*b"msf1");
                    have_image_sequence = true;
                } else {
                    match caps_structure.name().as_str() {
                        "video/x-h264" | "video/x-h265" | "video/x-vp8" | "video/x-vp9"
                        | "video/x-av1" | "image/jpeg" | "video/x-raw" => {
                            have_only_image_sequence = false;
                        }
                        _ => {}
                    }
                    match caps_structure.name().as_str() {
                        "video/x-h264" | "video/x-h265" | "video/x-vp8" | "video/x-vp9"
                        | "image/jpeg" | "video/x-raw" | "audio/mpeg" | "audio/x-opus"
                        | "audio/x-flac" | "audio/x-alaw" | "audio/x-mulaw" | "audio/x-adpcm"
                        | "audio/x-ac3" | "audio/x-eac3" => {
                            compatible_brands.insert(*b"mp41");
                            compatible_brands.insert(*b"mp42");
                            compatible_brands.insert(*b"isom");
                        }
                        _ => {}
                    }
                }

                match caps_structure.name().as_str() {
                    "video/x-av1" => {
                        minor_version = 1;
                        compatible_brands.insert(*b"av01");
                    }
                    "video/x-h264" => {
                        compatible_brands.insert(*b"avc1");
                    }
                    "audio/x-ac3" | "audio/x-eac3" => {
                        compatible_brands.insert(*b"dby1");
                    }
                    "audio/x-opus" => {
                        compatible_brands.insert(*b"opus");
                    }
                    _ => {}
                }
                if settings.with_precision_timestamps {
                    compatible_brands.insert(*b"iso6"); // required for saiz/saio support
                }
            }
            if have_image_sequence && have_only_image_sequence {
                major_brand = *b"msf1";
            }
            let settings = self.settings.lock().unwrap();
            for brand in &settings.extra_brands {
                compatible_brands.insert(*brand);
            }
            compatible_brands.insert(major_brand);
            // Convert BTreeSet to Vector
            let compatible_brands_vec: Vec<[u8; 4]> = compatible_brands.into_iter().collect();
            let ftyp =
                create_ftyp(major_brand, minor_version, compatible_brands_vec).map_err(|err| {
                    gst::error!(CAT, imp = self, "Failed to create ftyp box: {err}");
                    gst::FlowError::Error
                })?;
            state.current_offset += ftyp.size() as u64;
            buffers.get_mut().unwrap().add(ftyp);

            gst::info!(
                CAT,
                imp = self,
                "Creating mdat box header at offset {}",
                state.current_offset
            );
            state.mdat_offset = Some(state.current_offset);
            let mdat = create_mdat_header_non_frag(None).map_err(|err| {
                gst::error!(CAT, imp = self, "Failed to create mdat box header: {err}");
                gst::FlowError::Error
            })?;
            state.current_offset += mdat.size() as u64;
            state.mdat_size = 0;
            buffers.get_mut().unwrap().add(mdat);
        }

        let res = match self.drain_buffers(&settings, &mut state, buffers.get_mut().unwrap()) {
            Ok(_) => Ok(gst::FlowSuccess::Ok),
            Err(err @ gst::FlowError::Eos) | Err(err @ gst_base::AGGREGATOR_FLOW_NEED_DATA) => {
                Err(err)
            }
            Err(err) => return Err(err),
        };

        if res == Err(gst::FlowError::Eos) {
            // Create moov box now and append it to the buffers

            gst::info!(
                CAT,
                imp = self,
                "Creating moov box now, mdat ends at offset {} with size {}",
                state.current_offset,
                state.mdat_size
            );

            let min_earliest_pts = state
                .streams
                .iter()
                .filter_map(|s| s.earliest_pts)
                .min()
                .unwrap();
            let mut streams = Vec::with_capacity(state.streams.len());
            for stream in state.streams.drain(..) {
                let (earliest_pts, end_pts) = match Option::zip(stream.earliest_pts, stream.end_pts)
                {
                    Some(res) => res,
                    None => continue, // empty stream
                };

                streams.push(TrackConfiguration {
                    caps: stream.caps.clone(),
                    delta_frames: stream.delta_frames,
                    trak_timescale: stream.timescale(),
                    earliest_pts,
                    end_pts,
                    elst_infos: stream.get_elst_infos(min_earliest_pts).unwrap_or_else(|e| {
                        gst::error!(CAT, "Could not prepare edit lists: {e:?}");

                        Vec::new()
                    }),
                    image_sequence: stream.image_sequence_mode(),
                    extra_header_data: stream.extra_header_data.clone(),
                    language_code: stream.language_code,
                    orientation: stream.orientation(),
                    max_bitrate: stream.max_bitrate,
                    avg_bitrate: stream.avg_bitrate,
                    chunks: stream.chunks,
                    tai_clock_info: stream.tai_clock_info,
                    auxiliary_info: stream.aux_info,
                    codec_specific_boxes: stream.codec_specific_boxes.clone(),
                });
            }

            let moov = create_moov(
                PresentationConfiguration {
                    variant: self.obj().class().as_ref().variant,
                    movie_timescale: settings.movie_timescale,
                    tracks: streams,
                    // TODO: rework this
                    update: false,
                    write_mehd: false,
                    duration: None,
                    write_edts: false,
                },
                0,
                None,
                None,
            )
            .map_err(|err| {
                gst::error!(CAT, imp = self, "Failed to create moov box: {err}");
                gst::FlowError::Error
            })?;
            state.current_offset += moov.size() as u64;
            buffers.get_mut().unwrap().add(moov);
        }

        drop(state);

        if let Some(ref caps) = caps {
            self.obj().set_src_caps(caps);
        }

        if !buffers.is_empty() {
            if let Err(err) = self.obj().finish_buffer_list(buffers) {
                gst::error!(CAT, imp = self, "Failed pushing buffers: {err:?}");
                return Err(err);
            }
        }

        if res == Err(gst::FlowError::Eos) {
            let mut state = self.state.lock().unwrap();

            if let Some(mdat_offset) = state.mdat_offset {
                gst::info!(
                    CAT,
                    imp = self,
                    "Rewriting mdat box header at offset {mdat_offset} with size {} now",
                    state.mdat_size,
                );
                let mut segment = gst::FormattedSegment::<gst::format::Bytes>::new();
                segment.set_start(gst::format::Bytes::from_u64(mdat_offset));
                state.current_offset = mdat_offset;
                let mdat = create_mdat_header_non_frag(Some(state.mdat_size)).map_err(|err| {
                    gst::error!(CAT, imp = self, "Failed to create mdat box header: {err}");
                    gst::FlowError::Error
                })?;
                drop(state);

                self.obj().update_segment(&segment);
                if let Err(err) = self.obj().finish_buffer(mdat) {
                    gst::error!(
                        CAT,
                        imp = self,
                        "Failed pushing updated mdat box header buffer downstream: {err:?}",
                    );
                }
            }
        }

        res
    }
}

#[repr(C)]
pub(crate) struct Class {
    parent: gst_base::ffi::GstAggregatorClass,
    variant: Variant,
}

unsafe impl ClassStruct for Class {
    type Type = MP4Mux;
}

impl std::ops::Deref for Class {
    type Target = glib::Class<gst_base::Aggregator>;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(&self.parent as *const _ as *const _) }
    }
}

unsafe impl<T: MP4MuxImpl> IsSubclassable<T> for crate::isobmff::MP4Mux {
    fn class_init(class: &mut glib::Class<Self>) {
        Self::parent_class_init::<T>(class);

        let class = class.as_mut();
        class.variant = T::VARIANT;
    }
}

pub(crate) trait MP4MuxImpl:
    AggregatorImpl + ObjectSubclass<Type: IsA<crate::isobmff::MP4Mux>>
{
    const VARIANT: Variant;
}

#[derive(Default)]
pub(crate) struct ISOMP4Mux;

#[glib::object_subclass]
impl ObjectSubclass for ISOMP4Mux {
    const NAME: &'static str = "GstISOMP4Mux";
    type Type = crate::isobmff::ISOMP4Mux;
    type ParentType = crate::isobmff::MP4Mux;
}

impl ObjectImpl for ISOMP4Mux {}

impl GstObjectImpl for ISOMP4Mux {}

impl ElementImpl for ISOMP4Mux {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "ISOMP4Mux",
                "Codec/Muxer",
                "ISO MP4 muxer",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("video/quicktime")
                    .field("variant", "iso")
                    .build(),
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::with_gtype(
                "sink_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &[
                    gst::Structure::builder("video/x-h264")
                        .field("stream-format", gst::List::new(["avc", "avc3"]))
                        .field("alignment", "au")
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-h265")
                        .field("stream-format", gst::List::new(["hvc1", "hev1"]))
                        .field("alignment", "au")
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-vp8")
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-vp9")
                        .field("profile", gst::List::new(["0", "1", "2", "3"]))
                        .field("chroma-format", gst::List::new(["4:2:0", "4:2:2", "4:4:4"]))
                        .field("bit-depth-luma", gst::List::new([8u32, 10u32, 12u32]))
                        .field("bit-depth-chroma", gst::List::new([8u32, 10u32, 12u32]))
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-av1")
                        .field("stream-format", "obu-stream")
                        .field("alignment", "tu")
                        .field("profile", gst::List::new(["main", "high", "professional"]))
                        .field(
                            "chroma-format",
                            gst::List::new(["4:0:0", "4:2:0", "4:2:2", "4:4:4"]),
                        )
                        .field("bit-depth-luma", gst::List::new([8u32, 10u32, 12u32]))
                        .field("bit-depth-chroma", gst::List::new([8u32, 10u32, 12u32]))
                        .field("width", gst::IntRange::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-raw")
                        // TODO: this could be extended to handle gst_video::VideoMeta for non-default stride and plane offsets
                        .field(
                            "format",
                            // formats that do not use subsampling
                            // Plus NV12 and NV21 because that works OK with the interleaved planes
                            gst::List::new([
                                "IYU2",
                                "RGB",
                                "BGR",
                                "NV12",
                                "NV21",
                                "RGBA",
                                "ARGB",
                                "ABGR",
                                "BGRA",
                                "RGBx",
                                "BGRx",
                                "Y444",
                                "AYUV",
                                "GRAY8",
                                "GRAY16_BE",
                                "GBR",
                                "RGBP",
                                "BGRP",
                                "v308",
                                "r210",
                            ]),
                        )
                        .field("width", gst::IntRange::new(1, i32::MAX))
                        .field("height", gst::IntRange::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("video/x-raw")
                        // TODO: this could be extended to handle gst_video::VideoMeta for non-default stride and plane offsets
                        .field(
                            "format",
                            // Formats that use horizontal subsampling, but not vertical subsampling (4:2:2 and 4:1:1)
                            gst::List::new(["Y41B", "NV16", "NV61", "Y42B"]),
                        )
                        .field(
                            "width",
                            gst::IntRange::with_step(4, i32::MAX.prev_multiple_of(&4), 4),
                        )
                        .field("height", gst::IntRange::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("video/x-raw")
                        // TODO: this could be extended to handle gst_video::VideoMeta for non-default stride and plane offsets
                        .field(
                            "format",
                            // Formats that use both horizontal and vertical subsampling (4:2:0)
                            gst::List::new(["I420", "YV12", "YUY2", "YVYU", "UYVY", "VYUY"]),
                        )
                        .field(
                            "width",
                            gst::IntRange::with_step(4, i32::MAX.prev_multiple_of(&4), 4),
                        )
                        .field(
                            "height",
                            gst::IntRange::with_step(2, i32::MAX.prev_multiple_of(&2), 2),
                        )
                        .build(),
                    gst::Structure::builder("audio/mpeg")
                        .field("mpegversion", 4i32)
                        .field("stream-format", "raw")
                        .field("channels", gst::IntRange::new(1, u16::MAX as i32))
                        .field("rate", gst::IntRange::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("audio/x-opus")
                        .field("channel-mapping-family", gst::IntRange::new(0i32, 255))
                        .field("channels", gst::IntRange::new(1i32, 8))
                        .field("rate", gst::IntRange::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("audio/x-flac")
                        .field("framed", true)
                        .field("channels", gst::IntRange::<i32>::new(1, 8))
                        .field("rate", gst::IntRange::<i32>::new(1, 10 * u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("audio/x-ac3")
                        .field("framed", true)
                        .field("alignment", "frame")
                        .field("channels", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("rate", gst::IntRange::<i32>::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("audio/x-eac3")
                        .field("framed", true)
                        .field("alignment", "iec61937")
                        .field("channels", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("rate", gst::IntRange::<i32>::new(1, i32::MAX))
                        .build(),
                ]
                .into_iter()
                .collect::<gst::Caps>(),
                crate::isobmff::MP4MuxPad::static_type(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl AggregatorImpl for ISOMP4Mux {}

impl MP4MuxImpl for ISOMP4Mux {
    const VARIANT: Variant = Variant::ISO;
}

#[derive(Default)]
pub(crate) struct ONVIFMP4Mux;

#[glib::object_subclass]
impl ObjectSubclass for ONVIFMP4Mux {
    const NAME: &'static str = "GstONVIFMP4Mux";
    type Type = crate::isobmff::ONVIFMP4Mux;
    type ParentType = crate::isobmff::MP4Mux;
}

impl ObjectImpl for ONVIFMP4Mux {}

impl GstObjectImpl for ONVIFMP4Mux {}

impl ElementImpl for ONVIFMP4Mux {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "ONVIFMP4Mux",
                "Codec/Muxer",
                "ONVIF MP4 muxer",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &gst::Caps::builder("video/quicktime")
                    .field("variant", "iso")
                    .build(),
            )
            .unwrap();

            let sink_pad_template = gst::PadTemplate::with_gtype(
                "sink_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &[
                    gst::Structure::builder("video/x-h264")
                        .field("stream-format", gst::List::new(["avc", "avc3"]))
                        .field("alignment", "au")
                        .field("width", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("video/x-h265")
                        .field("stream-format", gst::List::new(["hvc1", "hev1"]))
                        .field("alignment", "au")
                        .field("width", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("image/jpeg")
                        .field("width", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("height", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .build(),
                    gst::Structure::builder("audio/mpeg")
                        .field("mpegversion", 4i32)
                        .field("stream-format", "raw")
                        .field("channels", gst::IntRange::<i32>::new(1, u16::MAX as i32))
                        .field("rate", gst::IntRange::<i32>::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("audio/x-alaw")
                        .field("channels", gst::IntRange::<i32>::new(1, 2))
                        .field("rate", gst::IntRange::<i32>::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("audio/x-mulaw")
                        .field("channels", gst::IntRange::<i32>::new(1, 2))
                        .field("rate", gst::IntRange::<i32>::new(1, i32::MAX))
                        .build(),
                    gst::Structure::builder("audio/x-adpcm")
                        .field("layout", "g726")
                        .field("channels", 1i32)
                        .field("rate", 8000i32)
                        .field("bitrate", gst::List::new([16000i32, 24000, 32000, 40000]))
                        .build(),
                    gst::Structure::builder("application/x-onvif-metadata")
                        .field("parsed", true)
                        .build(),
                ]
                .into_iter()
                .collect::<gst::Caps>(),
                crate::isobmff::MP4MuxPad::static_type(),
            )
            .unwrap();

            vec![src_pad_template, sink_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl AggregatorImpl for ONVIFMP4Mux {}

impl MP4MuxImpl for ONVIFMP4Mux {
    const VARIANT: Variant = Variant::ONVIF;
}

#[derive(Default, Clone)]
struct PadSettings {
    trak_timescale: u32,
    image_sequence_mode: bool,
}

#[derive(Default)]
pub(crate) struct MP4MuxPad {
    settings: Mutex<PadSettings>,
}

#[glib::object_subclass]
impl ObjectSubclass for MP4MuxPad {
    const NAME: &'static str = "GstRsMP4MuxPad";
    type Type = crate::isobmff::MP4MuxPad;
    type ParentType = gst_base::AggregatorPad;
}

impl ObjectImpl for MP4MuxPad {
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt::builder("trak-timescale")
                    .nick("Track Timescale")
                    .blurb("Timescale to use for the track (units per second, 0 is automatic)")
                    .mutable_ready()
                    .build(),
                glib::ParamSpecBoolean::builder("image-sequence")
                    .nick("Generate image sequence")
                    .blurb("Generate ISO/IEC 23008-12 image sequence instead of video")
                    .mutable_ready()
                    .build(),
            ]
        });

        &PROPERTIES
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "trak-timescale" => {
                let mut settings = self.settings.lock().unwrap();
                settings.trak_timescale = value.get().expect("type checked upstream");
            }

            "image-sequence" => {
                let mut settings = self.settings.lock().unwrap();
                settings.image_sequence_mode = value.get().expect("type checked upstream");
            }

            _ => unimplemented!(),
        }
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "trak-timescale" => {
                let settings = self.settings.lock().unwrap();
                settings.trak_timescale.to_value()
            }

            "image-sequence" => {
                let settings = self.settings.lock().unwrap();
                settings.image_sequence_mode.to_value()
            }

            _ => unimplemented!(),
        }
    }
}

impl GstObjectImpl for MP4MuxPad {}

impl PadImpl for MP4MuxPad {}

impl AggregatorPadImpl for MP4MuxPad {
    fn flush(&self, aggregator: &gst_base::Aggregator) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mux = aggregator.downcast_ref::<crate::isobmff::MP4Mux>().unwrap();
        let mut mux_state = mux.imp().state.lock().unwrap();

        gst::info!(CAT, imp = self, "Flushing");

        for stream in &mut mux_state.streams {
            if stream.sinkpad == *self.obj() {
                stream.pending_buffer = None;
                stream.pre_queue.clear();
                stream.running_time_utc_time_mapping = None;
                break;
            }
        }

        drop(mux_state);

        self.parent_flush(aggregator)
    }
}

impl ChildProxyImpl for MP4Mux {
    fn children_count(&self) -> u32 {
        let object = self.obj();
        object.num_sink_pads() as u32
    }

    fn child_by_name(&self, name: &str) -> Option<glib::Object> {
        let object = self.obj();
        object
            .sink_pads()
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
