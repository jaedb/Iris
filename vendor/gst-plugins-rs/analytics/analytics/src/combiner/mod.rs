// SPDX-License-Identifier: MPL-2.0
/**
 * SECTION:element-analyticscombiner
 * @see_also: analyticssplitter
 *
 * `analyticscombiner` is a generic, media-agnostic stream combiner element for analytics purposes.
 *
 * Buffers and serialized events of one or more streams are combined into batches of a specific
 * duration, which can be configured via the `batch-duration` property. The batches are pushed
 * downstream as empty buffers with the `GstAnalyticsBatchMeta`, which contains the original
 * data flow of each stream. The order of the streams inside the `GstAnalyticsBatchMeta` are
 * defined by the `index` property on each of the sink pads, which defaults to being the index of
 * all sink pads when sorted according to the pad name.
 *
 * The caps on the source pad are of type `multistream/x-analytics-batch` and contain the original
 * caps of each stream in the `streams` field in the same order as they appear in the
 * `GstAnalyticsBatchMeta`. Caps negotiation ensures that downstream can provide constraints for
 * each of the input streams.
 *
 * By default all buffers that start inside a batch are included in the output. Via the
 * `batch-strategy` property on the sink pads this behaviour can be modified.
 *
 * The intended usage is to follow `analyticscombiner` by one or more inference elements that
 * process the combined stream, attach additional meta on the buffers, and then pass through
 * `analyticssplitter` that then splits the combined stream into its original streams again.
 *
 * ## Usage
 *
 * ```shell
 * gst-launch-1.0 filesrc location=file-1.mp4 ! decodebin3 ! combiner.sink_0 \
 *     filesrc location=file-2.mp4 ! decodebin3 ! combiner.sink_1 \
 *     analyticscombiner name=combiner batch-duration=100000000
 *       sink_0::batch-strategy=first-in-batch-with-overlay \
 *       sink_1::batch-strategy=first-in-batch-with-overlay ! \
 *     inference-elements ! \
 *     analyticssplitter name=splitter \
 *     splitter.src_0_0 ! objectdetectionoverlay ! videoconvert ! autovideosink \
 *     splitter.src_0_1 ! objectdetectionoverlay ! videoconvert ! autovideosink
 * ```
 *
 * Since: plugins-rs-0.14.0
 */
use glib::prelude::*;

mod imp;

glib::wrapper! {
    pub struct AnalyticsCombiner(ObjectSubclass<imp::AnalyticsCombiner>) @extends gst_base::Aggregator, gst::Element, gst::Object, @implements gst::ChildProxy;
}

glib::wrapper! {
    pub struct AnalyticsCombinerSinkPad(ObjectSubclass<imp::AnalyticsCombinerSinkPad>) @extends gst_base::AggregatorPad, gst::Pad, gst::Object;
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "GstAnalyticsCombinerBatchStrategy")]
#[repr(C)]
pub enum BatchStrategy {
    #[default]
    #[enum_value(
        name = "All: Include all buffers that start inside the batch.",
        nick = "all"
    )]
    All,
    #[enum_value(
        name = "FirstInBatch: Include only the first buffer that starts inside the batch.",
        nick = "first-in-batch"
    )]
    FirstInBatch,
    #[enum_value(
        name = "FirstInBatchWithOverlap: Include only the first buffer that starts inside the batch unless there was a previously unused buffer at most half a batch duration earlier. If no buffer is available, allow taking a buffer up to half a batch duration later. The buffer closest to the batch start is included.",
        nick = "first-in-batch-with-overlap"
    )]
    FirstInBatchWithOverlap,
    #[enum_value(
        name = "LastInBatch: Include only the last buffer that starts inside the batch.",
        nick = "last-in-batch"
    )]
    LastInBatch,
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        use gst::prelude::*;

        BatchStrategy::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        AnalyticsCombinerSinkPad::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }

    gst::Element::register(
        Some(plugin),
        "analyticscombiner",
        gst::Rank::NONE,
        AnalyticsCombiner::static_type(),
    )
}
