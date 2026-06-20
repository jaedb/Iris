//! ADU Frame Deinterleaving Buffer

use std::collections::BTreeMap;

use super::{
    Adu, AduCompleteUnparsed, InterleavingSeqnum, MaybeSingleAduOrList, MpegAudioRobustDepayError,
    CAT,
};

/// ADU Frame Deinterleaving Buffer.
///
/// In some packet modes, non-consecutive ADUs might be grouped together,
/// which can limit the gap between ADUs in case of packet loss.
///
/// The Deinterleaving Buffer collects ADUs as they arrive and outputs
/// them in the expected order whenever possible.
///
/// See [Interleaving in RFC 5219](rfc-interleaving) & [deinterleaving pseudo code](deinterleaving-pseudo-code).
///
/// [rfc-interleaving]: https://www.rfc-editor.org/rfc/rfc5219.html#section-7
/// [deinterleaving-pseudo-code]: https://www.rfc-editor.org/rfc/rfc5219.html#appendix-B.2
#[derive(Debug, Default)]
pub struct DeinterleavingAduBuffer {
    adus: BTreeMap<u8, Adu>,
    last_inserted: Option<InterleavingSeqnum>,
    last_popped: Option<InterleavingSeqnum>,
}

impl DeinterleavingAduBuffer {
    #[inline]
    pub fn drain(&mut self, elem: &super::RtpMpegAudioRobustDepay) -> MaybeSingleAduOrList {
        let len = self.adus.len();
        match len {
            0 => MaybeSingleAduOrList::default(),
            1 => {
                let mut adus = MaybeSingleAduOrList::default();

                let adu = self.adus.pop_first().unwrap().1;
                self.handle_ready(elem, adu, &mut adus);

                adus
            }
            _ => {
                let mut list = MaybeSingleAduOrList::new_list(len);

                while let Some((_, adu)) = self.adus.pop_first() {
                    self.handle_ready(elem, adu, &mut list);
                }

                list
            }
        }
    }

    #[inline]
    pub fn flush(&mut self) {
        self.adus.clear();
        self.last_inserted = None;
        self.last_popped = None;
    }

    #[inline]
    fn handle_ready(
        &mut self,
        elem: &super::RtpMpegAudioRobustDepay,
        mut adu: Adu,
        ready_adus: &mut MaybeSingleAduOrList,
    ) {
        let isn = adu.interleaving_seqnum();

        // If no previous ADU => discont
        let discont = self.last_popped.is_none_or(|last_popped| {
            if isn == InterleavingSeqnum::default() && last_popped == isn {
                // not interleaved
                false
            } else if last_popped.cycle == isn.cycle {
                last_popped.index + 1 != isn.index
            } else {
                let expected_cycle = if last_popped.cycle < 7 {
                    last_popped.cycle + 1
                } else {
                    0
                };

                if expected_cycle != isn.cycle {
                    true
                } else {
                    isn.index != 0
                }
            }
        });

        if discont {
            gst::info!(CAT, obj = elem, "DISCONT for {adu}");
            adu.set_discont();
        }

        self.last_popped = Some(isn);
        ready_adus.push(adu);
    }

    #[track_caller]
    pub fn try_push_and_pop(
        &mut self,
        elem: &super::RtpMpegAudioRobustDepay,
        adu: AduCompleteUnparsed,
        ready_adus: &mut MaybeSingleAduOrList,
    ) -> Result<(), MpegAudioRobustDepayError> {
        let cur_isn = adu.interleaving_seqnum();

        // Got the index => can restore the MPEG syncword and parse the MP3 header
        let adu = Adu::try_from(adu, cur_isn)?;

        gst::trace!(CAT, obj = elem, "Pushing {adu}");

        if let Some(last_inserted) = self.last_inserted {
            if cur_isn.cycle != last_inserted.cycle || cur_isn.index == last_inserted.index {
                // > We've started a new interleave cycle
                // > (or interleaving was not used). Release all
                // > pending ADU frames to the ADU->MP3 conversion step.

                ready_adus.reserve(self.adus.len());

                while let Some((_, ready_adu)) = self.adus.pop_first() {
                    gst::trace!(CAT, obj = elem, "Popped {ready_adu}");

                    self.handle_ready(elem, ready_adu, ready_adus);
                }
            }
        }

        self.last_inserted = Some(cur_isn);
        self.adus.insert(cur_isn.index, adu);

        Ok(())
    }
}
