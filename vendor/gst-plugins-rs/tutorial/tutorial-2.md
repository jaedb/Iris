# How to write GStreamer Elements in Rust Part 2: A raw audio sine wave source

In this part, a raw audio sine wave source element is going to be written. The final code can be found [here](https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/-/tree/main/tutorial/src/sinesrc).

### Table of Contents

 1.  [Boilerplate](#boilerplate)
 2.  [Caps Negotiation](#caps-negotiation)
 3.  [Buffer Creation](#buffer-creation)
 4.  [(Pseudo) Live Mode](#pseudo-live-mode)
 5.  [Unlocking](#unlocking)
 6.  [Seeking](#seeking)

### Boilerplate

The first part here will be all the boilerplate required to set up the element. You can safely [skip](#caps-negotiation) this if you remember all this from the [previous tutorial](tutorial-1.md).

Our sine wave element is going to produce raw audio, with a number of channels and any possible sample rate with both 32 bit and 64 bit floating point samples. It will produce a simple sine wave with a configurable frequency, volume/mute and number of samples per audio buffer. In addition it will be possible to configure the element in (pseudo) live mode, meaning that it will only produce data in real-time according to the pipeline clock. And it will be possible to seek to any time/sample position on our source element. It will basically be a more simple version of the [`audiotestsrc`](https://gstreamer.freedesktop.org/documentation/audiotestsrc/index.html) element from gst-plugins-base.

So let's get started with all the boilerplate. This time our element will be based on the [`PushSrc`](https://gstreamer.freedesktop.org/documentation/base/gstpushsrc.html#GstPushSrc) base class instead of [`VideoFilter`](https://gstreamer.freedesktop.org/documentation/video/gstvideofilter.html#GstVideoFilter). `PushSrc` is a subclass of the [`BaseSrc`](https://gstreamer.freedesktop.org/documentation/base/gstbasesrc.html#GstBaseSrc) base class that only works in push mode, i.e. creates buffers as they arrive instead of allowing downstream elements to explicitly pull them.

In `src/sinesrc/imp.rs`:

```rust
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::prelude::*;
use gst_base::subclass::base_src::CreateSuccess;
use gst_base::subclass::prelude::*;

use byte_slice_cast::*;

use std::ops::Rem;
use std::sync::Mutex;
use std::{i32, u32};

use num_traits::cast::NumCast;
use num_traits::float::Float;

use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "rssinesrc",
        gst::DebugColorFlags::empty(),
        Some("Rust Sine Wave Source"),
    )
});

// Default values of properties
const DEFAULT_SAMPLES_PER_BUFFER: u32 = 1024;
const DEFAULT_FREQ: u32 = 440;
const DEFAULT_VOLUME: f64 = 0.8;
const DEFAULT_MUTE: bool = false;
const DEFAULT_IS_LIVE: bool = false;

// Property value storage
#[derive(Debug, Clone, Copy)]
struct Settings {
    samples_per_buffer: u32,
    freq: u32,
    volume: f64,
    mute: bool,
    is_live: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            samples_per_buffer: DEFAULT_SAMPLES_PER_BUFFER,
            freq: DEFAULT_FREQ,
            volume: DEFAULT_VOLUME,
            mute: DEFAULT_MUTE,
            is_live: DEFAULT_IS_LIVE,
        }
    }
}

// Stream-specific state, i.e. audio format configuration
// and sample offset
struct State {
    info: Option<gst_audio::AudioInfo>,
    sample_offset: u64,
    sample_stop: Option<u64>,
    accumulator: f64,
}

impl Default for State {
    fn default() -> State {
        State {
            info: None,
            sample_offset: 0,
            sample_stop: None,
            accumulator: 0.0,
        }
    }
}

// Struct containing all the element data
#[derive(Default)]
pub struct SineSrc {
    settings: Mutex<Settings>,
    state: Mutex<State>,
}

#[glib::object_subclass]
impl ObjectSubclass for SineSrc {
    const NAME: &'static str = "GstRsSineSrc";
    type Type = super::SineSrc;
    type ParentType = gst_base::PushSrc;
}

impl ObjectImpl for SineSrc {
    // Metadata for the properties
    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
            vec![
                glib::ParamSpecUInt::builder("samples-per-buffer")
                    .nick("Samples Per Buffer")
                    .blurb("Number of samples per output buffer")
                    .minimum(1)
                    .default_value(DEFAULT_SAMPLES_PER_BUFFER)
                    .mutable_ready()
                    .build(),
                glib::ParamSpecUInt::builder("freq")
                    .nick("Frequency")
                    .blurb("Frequency")
                    .minimum(1)
                    .default_value(DEFAULT_FREQ)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecDouble::builder("volume")
                    .nick("Volume")
                    .blurb("Output volume")
                    .maximum(10.0)
                    .default_value(DEFAULT_VOLUME)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecBoolean::builder("mute")
                    .nick("Mute")
                    .blurb("Mute")
                    .default_value(DEFAULT_MUTE)
                    .mutable_playing()
                    .build(),
                glib::ParamSpecBoolean::builder("is-live")
                    .nick("Is Live")
                    .blurb("(Pseudo) live output")
                    .default_value(DEFAULT_IS_LIVE)
                    .mutable_ready()
                    .build(),
            ]
        });

        PROPERTIES.as_ref()
    }

    // Called right after construction of a new instance
    fn constructed(&self) {
        // Call the parent class' ::constructed() implementation first
        self.parent_constructed();

        // Initialize live-ness and notify the base class that
        // we'd like to operate in Time format
        let obj = self.obj();
        obj.set_live(DEFAULT_IS_LIVE);
        obj.set_format(gst::Format::Time);
    }

    // Called whenever a value of a property is changed. It can be called
    // at any time from any thread.
    fn set_property(
        &self,
        _id: usize,
        value: &glib::Value,
        pspec: &glib::ParamSpec,
    ) {
        match pspec.name() {
            "samples-per-buffer" => {
                let mut settings = self.settings.lock().unwrap();
                let samples_per_buffer = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing samples-per-buffer from {} to {}",
                    settings.samples_per_buffer,
                    samples_per_buffer
                );
                settings.samples_per_buffer = samples_per_buffer;
                drop(settings);

                let _ = self.obj().post_message(gst::message::Latency::builder().src(&*self.obj()).build());
            }
            "freq" => {
                let mut settings = self.settings.lock().unwrap();
                let freq = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing freq from {} to {}",
                    settings.freq,
                    freq
                );
                settings.freq = freq;
            }
            "volume" => {
                let mut settings = self.settings.lock().unwrap();
                let volume = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing volume from {} to {}",
                    settings.volume,
                    volume
                );
                settings.volume = volume;
            }
            "mute" => {
                let mut settings = self.settings.lock().unwrap();
                let mute = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing mute from {} to {}",
                    settings.mute,
                    mute
                );
                settings.mute = mute;
            }
            "is-live" => {
                let mut settings = self.settings.lock().unwrap();
                let is_live = value.get().expect("type checked upstream");
                gst::info!(
                    CAT,
                    imp = self,
                    "Changing is-live from {} to {}",
                    settings.is_live,
                    is_live
                );
                settings.is_live = is_live;
            }
            _ => unimplemented!(),
        }
    }

    // Called whenever a value of a property is read. It can be called
    // at any time from any thread.
    fn get_property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "samples-per-buffer" => {
                let settings = self.settings.lock().unwrap();
                settings.samples_per_buffer.to_value()
            }
            "freq" => {
                let settings = self.settings.lock().unwrap();
                settings.freq.to_value()
            }
            "volume" => {
                let settings = self.settings.lock().unwrap();
                settings.volume.to_value()
            }
            "mute" => {
                let settings = self.settings.lock().unwrap();
                settings.mute.to_value()
            }
            "is-live" => {
                let settings = self.settings.lock().unwrap();
                settings.is_live.to_value()
            }
            _ => unimplemented!(),
        }
    }
}

// Virtual methods of gst::Element.
impl ElementImpl for SineSrc {
    // Set the element specific metadata. This information is what
    // is visible from gst-inspect-1.0 and can also be programmatically
    // retrieved from the gst::Registry after initial registration
    // without having to load the plugin in memory.
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
            gst::subclass::ElementMetadata::new(
                "Sine Wave Source",
                "Source/Audio",
                "Creates a sine wave",
                "Sebastian Dröge <sebastian@centricular.com>",
            )
        });

        Some(&*ELEMENT_METADATA)
    }
    
    // Create and add pad templates for our sink and source pad. These
    // are later used for actually creating the pads and beforehand
    // already provide information to GStreamer about all possible
    // pads that could exist for this type.
    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
            // On the src pad, we can produce F32/F64 with any sample rate
            // and any number of channels
            let caps = gst_audio::AudioCapsBuilder::new_interleaved()
                .format_list([gst_audio::AUDIO_FORMAT_F32, gst_audio::AUDIO_FORMAT_F64])
                .build();
            // The src pad template must be named "src" for basesrc
            // and specific a pad that is always there
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();

            vec![src_pad_template]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl BaseSrcImpl for SineSrc {
    // Called when starting, so we can initialize all stream-related state to its defaults
    fn start(&self) -> Result<(), gst::ErrorMessage> {
        // Reset state
        *self.state.lock().unwrap() = Default::default();

        gst::info!(CAT, imp = self, "Started");

        Ok(())
    }

    // Called when shutting down the element so we can release all stream-related state
    fn stop(&self) -> bool {
        // Reset state
        *self.state.lock().unwrap() = Default::default();

        gst::info!(CAT, imp = self, "Stopped");

        true
    }
}
```

In `src/sinesrc/mod.rs`:

```rust
use gst::glib;
use gst::prelude::*;

mod imp;

// The public Rust wrapper type for our element
glib::wrapper! {
    pub struct SineSrc(ObjectSubclass<imp::SineSrc>) @extends gst_base::BaseSrc, gst::Element, gst::Object;
}

// Registers the type for our element, and then registers in GStreamer under
// the name "rssinesrc" for being able to instantiate it via e.g.
// gst::ElementFactory::make().
pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "rssinesrc",
        gst::Rank::NONE,
        SineSrc::static_type(),
    )
}
```

If any of this needs explanation, please see the [previous](tutorial-1.md) and the comments in the code. The explanation for all the structs fields and what they're good for will follow in the next sections.

With all of the above and a small addition to `src/lib.rs` this should compile now.

```rust
mod sinesrc;
[...]

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    [...]
    sinesrc::register(plugin);
    Ok(())
}
```

Also a couple of new crates have to be added to `Cargo.toml` and `src/lib.rs`, but you best check the code in the [repository](https://gitlab.freedesktop.org/gstreamer/gst-plugins-rs/tree/main/tutorial) for details.

### Caps Negotiation

The first part that we have to implement, just like last time, is caps negotiation. We already notified the base class about any caps that we can potentially handle via the caps in the pad template returned from `pad_templates` function, but there are still two more steps of behaviour left that we have to implement.

First of all, we need to get notified whenever the caps that our source is configured for are changing. This will happen once in the very beginning and then whenever the pipeline topology or state changes and new caps would be more optimal for the new situation. This notification happens via the `BaseSrcImpl::set_caps` virtual method.

```rust
    fn set_caps(&self, caps: &gst::Caps) -> Result<(), gst::LoggableError> {
        use std::f64::consts::PI;

        let info = gst_audio::AudioInfo::from_caps(caps).map_err(|_| {
            gst::loggable_error!(CAT, "Failed to build `AudioInfo` from caps {}", caps)
        })?;

        gst::debug!(CAT, imp = self, "Configuring for caps {}", caps);

        self.obj().set_blocksize(info.bpf() * (*self.settings.lock().unwrap()).samples_per_buffer);

        let settings = *self.settings.lock().unwrap();
        let mut state = self.state.lock().unwrap();

        // If we have no caps yet, any old sample_offset and sample_stop will be
        // in nanoseconds
        let old_rate = match state.info {
            Some(ref info) => info.rate() as u64,
            None => *gst::ClockTime::SECOND,
        };

        // Update sample offset and accumulator based on the previous values and the
        // sample rate change, if any
        let old_sample_offset = state.sample_offset;
        let sample_offset = old_sample_offset
            .mul_div_floor(info.rate() as u64, old_rate)
            .unwrap();

        let old_sample_stop = state.sample_stop;
        let sample_stop =
            old_sample_stop.map(|v| v.mul_div_floor(info.rate() as u64, old_rate).unwrap());

        let accumulator =
            (sample_offset as f64).rem(2.0 * PI * (settings.freq as f64) / (info.rate() as f64));

        *state = State {
            info: Some(info),
            sample_offset: sample_offset,
            sample_stop: sample_stop,
            accumulator: accumulator,
        };

        drop(state);

        let _ = self.obj().post_message(gst::message::Latency::builder().src(&*self.obj()).build());

        Ok(())
    }
```

In here we parse the caps into a [`AudioInfo`](https://gstreamer.pages.freedesktop.org/gstreamer-rs/stable/latest/docs/gstreamer_audio/struct.AudioInfo.html) and then store that in our internal state, while updating various fields. We tell the base class about the number of bytes each buffer is usually going to hold, and update our current sample position, the stop sample position (when a seek with stop position happens, we need to know when to stop) and our accumulator. This happens by scaling both positions by the old and new sample rate. If we don't have an old sample rate, we assume nanoseconds (this will make more sense once seeking is implemented). The scaling is done with the help of the [`muldiv`](https://crates.io/crates/muldiv) crate, which implements scaling of integer types by a fraction with protection against overflows by doing up to 128 bit integer arithmetic for intermediate values.

The accumulator is then updated based on the current phase of the sine wave at the current sample position.

As a last step we post a new `LATENCY` message on the bus whenever the sample rate has changed. Our latency (in live mode) is going to be the duration of a single buffer, but more about that later.

`BaseSrc` is by default already selecting possible caps for us, if there are multiple options. However these defaults might not be (and often are not) ideal and we should override the default behaviour slightly. This is done in the `BaseSrc::fixate` virtual method.

```rust
    fn fixate(&self, mut caps: gst::Caps) -> gst::Caps {
        // Fixate the caps. BaseSrc will do some fixation for us, but
        // as we allow any rate between 1 and MAX it would fixate to 1. 1Hz
        // is generally not a useful sample rate.
        //
        // We fixate to the closest integer value to 48kHz that is possible
        // here, and for good measure also decide that the closest value to 1
        // channel is good.
        caps.truncate();
        {
            let caps = caps.make_mut();
            let s = caps.structure_mut(0).unwrap();
            s.fixate_field_nearest_int("rate", 48_000);
            s.fixate_field_nearest_int("channels", 1);
        }

        // Let BaseSrc fixate anything else for us. We could've alternatively have
        // called caps.fixate() here
        self.parent_fixate(caps)
    }
```

Here we take the caps that are passed in, truncate them (i.e. remove all but the very first [`Structure`](https://gstreamer.pages.freedesktop.org/gstreamer-rs/stable/latest/docs/gstreamer/structure/struct.Structure.html)) and then manually fixate the sample rate to the closest value to 48kHz. By default, caps fixation would result in the lowest possible sample rate but this is usually not desired.

For good measure, we also fixate the number of channels to the closest value to 1, but this would already be the default behaviour anyway. And then chain up to the parent class' implementation of `fixate`, which for now basically does the same as `caps.fixate()`. After this, the caps are fixated, i.e. there is only a single `Structure` left and all fields have concrete values (no ranges or sets).

### Buffer Creation

Now we have everything in place for a working element, apart from the virtual method to actually generate the raw audio buffers with the sine wave. From a high-level `BaseSrc` works by calling the `create` virtual method over and over again to let the subclass produce a buffer until it returns an error or signals the end of the stream.

Let's first talk about how to generate the sine wave samples themselves. As we want to operate on 32 bit and 64 bit floating point numbers, we implement a generic function for generating samples and storing them in a mutable byte slice. This is done with the help of the [`num_traits`](https://crates.io/crates/num-traits) crate, which provides all kinds of useful traits for abstracting over numeric types. In our case we only need the [`Float`](https://docs.rs/num-traits/0.2.0/num_traits/float/trait.Float.html) and [`NumCast`](https://docs.rs/num-traits/0.2.0/num_traits/cast/trait.NumCast.html) traits.

Instead of writing a generic implementation with those traits, it would also be possible to do the same with a simple macro that generates a function for both types. Which approach is nicer is a matter of taste in the end, the compiler output should be equivalent for both cases.

```rust
    fn process<F: Float + FromByteSlice>(
        data: &mut [u8],
        accumulator_ref: &mut f64,
        freq: u32,
        rate: u32,
        channels: u32,
        vol: f64,
    ) {
        use std::f64::consts::PI;

        // Reinterpret our byte-slice as a slice containing elements of the type
        // we're interested in. GStreamer requires for raw audio that the alignment
        // of memory is correct, so this will never ever fail unless there is an
        // actual bug elsewhere.
        let data = data.as_mut_slice_of::<F>().unwrap();

        // Convert all our parameters to the target type for calculations
        let vol: F = NumCast::from(vol).unwrap();
        let freq = freq as f64;
        let rate = rate as f64;
        let two_pi = 2.0 * PI;

        // We're carrying a accumulator with up to 2pi around instead of working
        // on the sample offset. High sample offsets cause too much inaccuracy when
        // converted to floating point numbers and then iterated over in 1-steps
        let mut accumulator = *accumulator_ref;
        let step = two_pi * freq / rate;

        for chunk in data.chunks_mut(channels as usize) {
            let value = vol * F::sin(NumCast::from(accumulator).unwrap());
            for sample in chunk {
                *sample = value;
            }

            accumulator += step;
            if accumulator >= two_pi {
                accumulator -= two_pi;
            }
        }

        *accumulator_ref = accumulator;
    }
```

This function takes the mutable byte slice from our buffer as argument, as well as the current value of the accumulator and the relevant settings for generating the sine wave.

As a first step, we "cast" the byte slice to one of the target type (f32 or f64) with the help of the [`byte_slice_cast`](https://crates.io/crates/byte-slice-cast) crate. This ensures that alignment and sizes are all matching and returns a mutable slice of our target type if successful. In case of GStreamer, the buffer alignment is guaranteed to be big enough for our types here and we allocate the buffer of a correct size later.

Now we convert all the parameters to the types we will use later, and store them together with the current accumulator value in local variables. Then we iterate over the whole floating point number slice in chunks with all channels, and fill each channel with the current value of our sine wave.

The sine wave itself is calculated by `val = volume * sin(2 * PI * frequency * (i + accumulator) / rate)`, but we actually calculate it by simply increasing the accumulator by `2 * PI * frequency / rate` for every sample instead of doing the multiplication for each sample. We also make sure that the accumulator always stays between `0` and `2 * PI` to prevent any inaccuracies from floating point numbers to affect our produced samples.

Now that this is done, we need to implement the `PushSrc::create` virtual method for actually allocating the buffer, setting timestamps and other metadata and it and calling our above function.

```rust
    fn create(
        &self,
        _buffer: Option<&mut gst::BufferRef>,
    ) -> Result<CreateSuccess, gst::FlowError> {
        // Keep a local copy of the values of all our properties at this very moment. This
        // ensures that the mutex is never locked for long and the application wouldn't
        // have to block until this function returns when getting/setting property values
        let settings = *self.settings.lock().unwrap();

        // Get a locked reference to our state, i.e. the input and output AudioInfo
        let mut state = self.state.lock().unwrap();
        let info = match state.info {
            None => {
                gst::element_imp_error!(self, gst::CoreError::Negotiation, ["Have no caps yet"]);
                return Err(gst::FlowError::NotNegotiated);
            }
            Some(ref info) => info.clone(),
        };

        // If a stop position is set (from a seek), only produce samples up to that
        // point but at most samples_per_buffer samples per buffer
        let n_samples = if let Some(sample_stop) = state.sample_stop {
            if sample_stop <= state.sample_offset {
                gst::log!(CAT, imp = self, "At EOS");
                return Err(gst::FlowError::Eos);
            }

            sample_stop - state.sample_offset
        } else {
            settings.samples_per_buffer as u64
        };

        // Allocate a new buffer of the required size, update the metadata with the
        // current timestamp and duration and then fill it according to the current
        // caps
        let mut buffer =
            gst::Buffer::with_size((n_samples as usize) * (info.bpf() as usize)).unwrap();
        {
            let buffer = buffer.get_mut().unwrap();

            // Calculate the current timestamp (PTS) and the next one,
            // and calculate the duration from the difference instead of
            // simply the number of samples to prevent rounding errors
            let pts = state
                .sample_offset
                .mul_div_floor(gst::ClockTime::SECOND, info.rate() as u64)
                .map(gst::ClockTime::from_nseconds)
                .unwrap();
            let next_pts = (state.sample_offset + n_samples)
                .mul_div_floor(*gst::ClockTime::SECOND, info.rate() as u64)
                .map(gst::ClockTime::from_nseconds)
                .unwrap();
            buffer.set_pts(pts);
            buffer.set_duration(next_pts - pts);

            // Map the buffer writable and create the actual samples
            let mut map = buffer.map_writable().unwrap();
            let data = map.as_mut_slice();

            if info.format() == gst_audio::AUDIO_FORMAT_F32 {
                Self::process::<f32>(
                    data,
                    &mut state.accumulator,
                    settings.freq,
                    info.rate(),
                    info.channels(),
                    settings.volume,
                );
            } else {
                Self::process::<f64>(
                    data,
                    &mut state.accumulator,
                    settings.freq,
                    info.rate(),
                    info.channels(),
                    settings.volume,
                );
            }
        }
        state.sample_offset += n_samples;
        drop(state);

        gst::debug!(CAT, imp = self, "Produced buffer {:?}", buffer);

        Ok(CreateSuccess::NewBuffer(buffer))
    }
```

Just like last time, we start with creating a copy of our properties (settings) and keeping a mutex guard of the internal state around. If the internal state has no `AudioInfo` yet, we error out. This would mean that no caps were negotiated yet, which is something we can't handle and is not really possible in our case.

Next we calculate how many samples we have to generate. If a sample stop position was set by a seek event, we have to generate samples up to at most that point. Otherwise we create at most the number of samples per buffer that were set via the property. Then we allocate a buffer of the corresponding size, with the help of the `bpf` field of the `AudioInfo`, and then set its metadata and fill the samples.

The metadata that is set is the timestamp (PTS), and the duration. The duration is calculated from the difference of the following buffer's timestamp and the current buffer's. By this we ensure that rounding errors are not causing the next buffer's timestamp to have a different timestamp than the sum of the current's and its duration. While this would not be much of a problem in GStreamer (inaccurate and jitterish timestamps are handled just fine), we can prevent it here and do so.

Afterwards we call our previously defined function on the writably mapped buffer and fill it with the sample values.

With all this, the element should already work just fine in any GStreamer-based application, for example `gst-launch-1.0`. Don't forget to set the `GST_PLUGIN_PATH` environment variable correctly like last time. Before running this, make sure to turn down the volume of your speakers/headphones a bit.

```bash
export GST_PLUGIN_PATH=`pwd`/target/debug
gst-launch-1.0 rssinesrc freq=440 volume=0.9 ! audioconvert ! autoaudiosink
```

You should hear a 440Hz sine wave now.

### (Pseudo) Live Mode

Many audio (and video) sources can actually only produce data in real-time and data is produced according to some clock. So far our source element can produce data as fast as downstream is consuming data, but we optionally can change that. We simulate a live source here now by waiting on the pipeline clock, but with a real live source you would only ever be able to have the data in real-time without any need to wait on a clock. And usually that data is produced according to a different clock than the pipeline clock, in which case translation between the two clocks is needed but we ignore this aspect for now. For details check the [GStreamer documentation](https://gstreamer.freedesktop.org/documentation/application-development/advanced/clocks.html).

For working in live mode, we have to add a few different parts in various places. First of all, we implement waiting on the clock in the `create` function.

```rust
    fn create(...)
        [...]
        state.sample_offset += n_samples;
        drop(state);

        // If we're live, we are waiting until the time of the last sample in our buffer has
        // arrived. This is the very reason why we have to report that much latency.
        // A real live-source would of course only allow us to have the data available after
        // that latency, e.g. when capturing from a microphone, and no waiting from our side
        // would be necessary.
        //
        // Waiting happens based on the pipeline clock, which means that a real live source
        // with its own clock would require various translations between the two clocks.
        // This is out of scope for the tutorial though.
        if self.obj().is_live() {
            let clock = match self.obj().clock() {
                None => return Ok(CreateSuccess::NewBuffer(buffer)),
                Some(clock) => clock,
            };

            let segment = self.obj().segment().downcast::<gst::format::Time>().unwrap();
            let base_time = self.obj().base_time();
            let running_time = segment.to_running_time(
                buffer
                    .pts()
                    .opt_add(buffer.duration()),
            );

            // The last sample's clock time is the base time of the element plus the
            // running time of the last sample
            let wait_until = match running_time.opt_add(base_time) {
                Some(wait_until) => wait_until,
                None => return Ok(CreateSuccess::NewBuffer(buffer)),
            };

            let id = clock.new_single_shot_id(wait_until);

            gst::log!(
                CAT,
                imp = self,
                "Waiting until {}, now {}",
                wait_until,
                clock.time(),
            );
            let (res, jitter) = id.wait();
            gst::log!(
                CAT,
                imp = self,
                "Waited res {:?} jitter {}",
                res,
                jitter
            );
        }

        gst::debug!(CAT, imp = self, "Produced buffer {:?}", buffer);

        Ok(CreateSuccess::NewBuffer(buffer))
    }
```

To be able to wait on the clock, we first of all need to calculate the clock time until when we want to wait. In our case that will be the clock time right after the end of the last sample in the buffer we just produced. Simply because you can't capture a sample before it was produced.

We calculate the running time from the PTS and duration of the buffer with the help of the currently configured segment and then add the base time of the element on this to get the clock time as result. Please check the [GStreamer documentation](https://gstreamer.freedesktop.org/documentation/application-development/advanced/clocks.html) for details, but in short the running time of a pipeline is the time since the start of the pipeline (or the last reset of the running time) and the running time of a buffer can be calculated from its PTS and the segment, which provides the information to translate between the two. The base time is the clock time when the pipeline went to the `Playing` state, so just an offset.

Next we wait and then return the buffer as before.

Now we also have to tell the base class that we're running in live mode now. This is done by calling `set_live(true)` on the base class before changing the element state from `Ready` to `Paused`. For this we override the `Element::change_state` virtual method.

```rust
impl ElementImpl for SineSrc {
    fn change_state(
        &self,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        // Configure live'ness once here just before starting the source
        match transition {
            gst::StateChange::ReadyToPaused => {
                self.obj().set_live(self.settings.lock().unwrap().is_live);
            }
            _ => (),
        }

        self.parent_change_state(transition)
    }
}
```

And as a last step, we also need to notify downstream elements about our [latency](https://gstreamer.freedesktop.org/documentation/application-development/advanced/clocks.html#latency). Live elements always have to report their latency so that synchronization can work correctly. As the clock time of each buffer is equal to the time when it was created, all buffers would otherwise arrive late in the sinks (they would appear as if they should've been played already at the time when they were created). So all the sinks will have to compensate for the latency that it took from capturing to the sink, and they have to do that in a coordinated way (otherwise audio and video would be out of sync if both have different latencies). For this the pipeline is querying each sink for the latency on its own branch, and then configures a global latency on all sinks according to that.

This querying is done with the `LATENCY` query, which we will have to handle in the `BaseSrc::query()` function.

```rust
    fn query(&self, query: &mut gst::QueryRef) -> bool {
        use gst::QueryViewMut;

        match query.view_mut() {
            // In Live mode we will have a latency equal to the number of samples in each buffer.
            // We can't output samples before they were produced, and the last sample of a buffer
            // is produced that much after the beginning, leading to this latency calculation
            QueryViewMut::Latency(q) => {
                let settings = *self.settings.lock().unwrap();
                let state = self.state.lock().unwrap();

                if let Some(ref info) = state.info {
                    let latency = gst::ClockTime::SECOND
                        .mul_div_floor(settings.samples_per_buffer as u64, info.rate() as u64)
                        .unwrap();
                    gst::debug!(CAT, imp = self, "Returning latency {}", latency);
                    q.set(settings.is_live, latency, gst::ClockTime::NONE);
                    true
                } else {
                    false
                }
            }
            _ => BaseSrcImplExt::parent_query(self, query),
        }

    }
```

The latency that we report is the duration of a single audio buffer, because we're simulating a real live source here. A real live source won't be able to output the buffer before the last sample of it is captured, and the difference between when the first and last sample were captured is exactly the latency that we add here. Other elements further downstream that introduce further latency would then add their own latency on top of this.

Inside the latency query we also signal that we are indeed a live source, and additionally how much buffering we can do (in our case, infinite) until data would be lost. The last part is important if e.g. the video branch has a higher latency, causing the audio sink to have to wait some additional time (so that audio and video stay in sync), which would then require the whole audio branch to buffer some data. As we have an artificial live source, we can always generate data for the next time but a real live source would only have a limited buffer and if no data is read and forwarded once that runs full, data would get lost.

You can test this again with e.g. `gst-launch-1.0` by setting the `is-live`property to true. It should write in the output now that the pipeline is live.

`audiotestsrc` element also does it via [`get_times`](https://gstreamer.freedesktop.org/documentation/base/gstbasesink.html?gi-language=c#GstBaseSinkClass::get_times) virtual method. But as this is only really useful for pseudo live sources like this one, we decided to explain how waiting on the clock can be achieved correctly and even more important how that relates to the next section.

### Unlocking

With the addition of the live mode, the `create` function is now blocking and waiting on the clock for some time. This is suboptimal as for example a (flushing) seek would have to wait now until the clock waiting is done, or when shutting down the application would have to wait.

To prevent this, all waiting/blocking in GStreamer streaming threads should be interruptible/cancellable when requested. And for example the `ClockID` that we got from the clock for waiting can be cancelled by calling `unschedule()` on it. We only have to do it from the right place and keep it accessible. The right place is the `BaseSrc::unlock` virtual method.

```rust
struct ClockWait {
    clock_id: Option<gst::SingleShotClockId>,
    flushing: bool,
}

impl Default for ClockWait {
    fn default() -> Self {
        ClockWait {
            clock_id: None,
            flushing: true,
        }
    }
}

#[derive(Default)]
struct SineSrc {
    settings: Mutex<Settings>,
    state: Mutex<State>,
    clock_wait: Mutex<ClockWait>,
}

[...]

    fn unlock(&self) -> Result<(), gst::ErrorMessage> {
        // This should unblock the create() function ASAP, so we
        // just unschedule the clock it here, if any.
        gst::debug!(CAT, imp = self, "Unlocking");
        let mut clock_wait = self.clock_wait.lock().unwrap();
        if let Some(clock_id) = clock_wait.clock_id.take() {
            clock_id.unschedule();
        }
        clock_wait.flushing = true;

        Ok(())
    }
```

We store the clock ID in our struct, together with a boolean to signal whether we're supposed to flush already or not. And then inside `unlock`unschedule the clock ID and set this boolean flag to true.

Once everything is unlocked, we need to reset things again so that data flow can happen in the future. This is done in the `unlock_stop` virtual method.

```rust
    fn unlock_stop(&self) -> Result<(), gst::ErrorMessage> {
        // This signals that unlocking is done, so we can reset
        // all values again.
        gst::debug!(CAT, imp = self, "Unlock stop");
        let mut clock_wait = self.clock_wait.lock().unwrap();
        clock_wait.flushing = false;

        Ok(())
    }
```

To make sure that this struct is always initialized correctly, we also call `unlock` from `stop`, and `unlock_stop` from `start`.

Now as a last step, we need to actually make use of the new struct we added around the code where we wait for the clock.

```rust
            // Store the clock ID in our struct unless we're flushing anyway.
            // This allows to asynchronously cancel the waiting from unlock()
            // so that we immediately stop waiting on e.g. shutdown.
            let mut clock_wait = self.clock_wait.lock().unwrap();
            if clock_wait.flushing {
                gst::debug!(CAT, imp = self, "Flushing");
                return Err(gst::FlowError::Flushing);
            }

            let id = clock.new_single_shot_id(wait_until);
            clock_wait.clock_id = Some(id.clone());
            drop(clock_wait);

            gst::log!(
                CAT,
                imp = self,
                "Waiting until {}, now {}",
                wait_until,
                clock.time(),
            );
            let (res, jitter) = id.wait();
            gst::log!(
                CAT,
                imp = self,
                "Waited res {:?} jitter {}",
                res,
                jitter
            );
            self.clock_wait.lock().unwrap().clock_id.take();

            // If the clock ID was unscheduled, unlock() was called
            // and we should return Flushing immediately.
            if res == Err(gst::ClockError::Unscheduled) {
                gst::debug!(CAT, imp = self, "Flushing");
                return Err(gst::FlowError::Flushing);
            }
```

The important part in this code is that we first have to check if we are already supposed to unlock, before even starting to wait. Otherwise we would start waiting without anybody ever being able to unlock. Then we need to store the clock id in the struct and make sure to drop the mutex guard so that the `unlock` function can take it again for unscheduling the clock ID. And once waiting is done, we need to remove the clock id from the struct again and in case of `ClockError::Unscheduled` we directly return `Err(gst::FlowError::Flushing)` instead.

Similarly when using other blocking APIs it is important that they are woken up in a similar way when `unlock` is called. Otherwise the application developer's and thus user experience will be far from ideal.

### Seeking

As the last feature we implement seeking on our source element. In our case that only means that we have to update the `sample_offset` and `sample_stop` fields accordingly, other sources might have to do more work than that.

Seeking is implemented in the `BaseSrc::do_seek` virtual method, and signalling whether we can actually seek in the `is_seekable` virtual method.

```rust
    fn is_seekable(&self) -> bool {
        true
    }

    fn do_seek(&self, segment: &mut gst::Segment) -> bool {
        // Handle seeking here. For Time and Default (sample offset) seeks we can
        // do something and have to update our sample offset and accumulator accordingly.
        //
        // Also we should remember the stop time (so we can stop at that point), and if
        // reverse playback is requested. These values will all be used during buffer creation
        // and for calculating the timestamps, etc.

        if segment.rate() < 0.0 {
            gst::error!(CAT, imp = self, "Reverse playback not supported");
            return false;
        }

        let settings = *self.settings.lock().unwrap();
        let mut state = self.state.lock().unwrap();

        // We store sample_offset and sample_stop in nanoseconds if we
        // don't know any sample rate yet. It will be converted correctly
        // once a sample rate is known.
        let rate = match state.info {
            None => *gst::ClockTime::SECOND,
            Some(ref info) => info.rate() as u64,
        };

        if let Some(segment) = segment.downcast_ref::<gst::format::Time>() {
            use std::f64::consts::PI;

            let sample_offset = segment
                .start()
                .unwrap()
                .nseconds()
                .mul_div_floor(rate, *gst::ClockTime::SECOND)
                .unwrap();

            let sample_stop = segment
                .stop()
                .map(|v| v.nseconds().mul_div_floor(rate, *gst::ClockTime::SECOND).unwrap());

            let accumulator =
                (sample_offset as f64).rem(2.0 * PI * (settings.freq as f64) / (rate as f64));

            gst::debug!(
                CAT,
                imp = self,
                "Seeked to {}-{:?} (accum: {}) for segment {:?}",
                sample_offset,
                sample_stop,
                accumulator,
                segment
            );

            *state = State {
                info: state.info.clone(),
                sample_offset,
                sample_stop,
                accumulator,
            };

            true
        } else if let Some(segment) = segment.downcast_ref::<gst::format::Default>() {
            use std::f64::consts::PI;

            if state.info.is_none() {
                gst::error!(
                    CAT,
                    imp = self,
                    "Can only seek in Default format if sample rate is known"
                );
                return false;
            }

            let sample_offset = *segment.start().unwrap();
            let sample_stop = segment.stop().map(|stop| *stop);

            let accumulator =
                (sample_offset as f64).rem(2.0 * PI * (settings.freq as f64) / (rate as f64));

            gst::debug!(
                CAT,
                imp = self,
                "Seeked to {}-{:?} (accum: {}) for segment {:?}",
                sample_offset,
                sample_stop,
                accumulator,
                segment
            );

            *state = State {
                info: state.info.clone(),
                sample_offset,
                sample_stop,
                accumulator,
            };

            true
        } else {
            gst::error!(
                CAT,
                imp = self,
                "Can't seek in format {:?}",
                segment.format()
            );

            false
        }
    }
```

Currently no support for reverse playback is implemented here, that is left as an exercise for the reader. So as a first step we check if the segment has a negative rate, in which case we just fail and return false.

Afterwards we again take a copy of the settings, keep a mutable mutex guard of our state and then start handling the actual seek.

If no caps are known yet, i.e. the `AudioInfo` is `None`, we assume a rate of 1 billion. That is, we just store the time in nanoseconds for now and let the `set_caps` function take care of that (which we already implemented accordingly) once the sample rate is known.

Then, if a `Time` seek is performed, we convert the segment start and stop position from time to sample offsets and save them. And then update the accumulator in a similar way as in the `set_caps` function. If a seek is in `Default` format (i.e. sample offsets for raw audio), we just have to store the values and update the accumulator but only do so if the sample rate is known already. A sample offset seek does not make any sense until the sample rate is known, so we just fail here to prevent unexpected surprises later.

Try the following pipeline for testing seeking. You should be able to seek the current time drawn over the video, and with the left/right cursor key you can seek. Also this shows that we create a quite nice sine wave.

```bash
gst-launch-1.0  rssinesrc  !  audioconvert  !  monoscope  !  timeoverlay  !  navseek  !  glimagesink
```

And with that all features are implemented in our sine wave raw audio source.
