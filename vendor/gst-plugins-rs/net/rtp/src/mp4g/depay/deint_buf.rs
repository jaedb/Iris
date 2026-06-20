//! Access Unit Deinterleaving Buffer
use slab::Slab;

use super::{AccessUnit, AccessUnitIndex, MaybeSingleAuOrList, Mpeg4GenericDepayError};

#[derive(Debug)]
struct AuNode {
    au: AccessUnit,
    /// Index of the next AuNode in the early_aus buffer
    next: Option<usize>,
}

/// Access Unit Deinterleaving Buffer.
///
/// In some packet modes, non-consecutive AUs might be grouped together,
/// which can limit the gap between to AUs in case of packet loss.
///
/// The Deinterleaving Buffer collects AUs as they arrive and outputs
/// them in the expected order whenever possible.
///
/// See [Interleaving in RFC 3640](rfc-interleaving).
///
/// [rfc-interleaving]: https://www.rfc-editor.org/rfc/rfc3640.html#section-3.2.3.2
#[derive(Debug, Default)]
pub struct DeinterleaveAuBuffer {
    /// Linked list of the early AUs
    early_aus: Slab<AuNode>,
    /// Index of the head in early_aus buffer
    head: Option<usize>,
    expected_index: Option<AccessUnitIndex>,
}

impl DeinterleaveAuBuffer {
    pub fn new(max_displacement: u32) -> Self {
        DeinterleaveAuBuffer {
            early_aus: Slab::with_capacity(max_displacement as usize),
            ..Default::default()
        }
    }

    pub fn drain(&mut self) -> MaybeSingleAuOrList {
        self.expected_index = None;

        let mut cur_opt = self.head.take();

        let len = self.early_aus.len();
        match len {
            0 => return MaybeSingleAuOrList::default(),
            1 => {
                let node = self.early_aus.remove(cur_opt.unwrap());
                return MaybeSingleAuOrList::from(node.au);
            }
            _ => (),
        }

        let mut list = MaybeSingleAuOrList::new_list(len);

        while let Some(cur) = cur_opt {
            let cur_node = self.early_aus.remove(cur);
            list.push(cur_node.au);

            cur_opt = cur_node.next;
        }

        list
    }

    pub fn flush(&mut self) {
        self.early_aus.clear();
        self.head = None;
        self.expected_index = None;
    }

    #[track_caller]
    pub fn push_and_pop(
        &mut self,
        au: AccessUnit,
        outbuf: &mut MaybeSingleAuOrList,
    ) -> Result<(), Mpeg4GenericDepayError> {
        use std::cmp::Ordering::*;

        let mut expected_index = match self.expected_index {
            Some(expected_index) => match au.index.try_cmp(expected_index)? {
                Equal => expected_index,
                Greater => return self.insert_au(au),
                Less => {
                    // Dropping too early Au
                    return Err(Mpeg4GenericDepayError::TooEarlyAU {
                        index: au.index,
                        expected_index,
                    });
                }
            },
            None => au.index, // first AU
        };

        outbuf.push(au);

        expected_index += 1;
        self.expected_index = Some(expected_index);

        // Pop other ready AUs if any
        let mut head;
        let mut head_node_ref;
        let mut head_node;
        while !self.early_aus.is_empty() {
            head = self.head.expect("!early_aus.is_empty");

            head_node_ref = self.early_aus.get(head).unwrap();
            if head_node_ref.au.index.try_cmp(expected_index)?.is_ne() {
                break;
            }

            head_node = self.early_aus.remove(head);
            outbuf.push(head_node.au);

            expected_index += 1;
            self.expected_index = Some(expected_index);

            self.head = head_node.next;
        }

        Ok(())
    }

    fn insert_au(&mut self, au: AccessUnit) -> Result<(), Mpeg4GenericDepayError> {
        use std::cmp::Ordering::*;

        if self.early_aus.is_empty() {
            self.head = Some(self.early_aus.insert(AuNode { au, next: None }));

            // Nothing to pop
            return Ok(());
        }

        let mut cur = self.head.expect("!early_aus.is_empty");
        let mut cur_node = self.early_aus.get(cur).unwrap();

        // cur & cur_node refer to current head here
        match au.index.try_cmp(cur_node.au.index)? {
            Greater => (),
            Less => {
                // New head
                self.head = Some(self.early_aus.insert(AuNode {
                    au,
                    next: Some(cur),
                }));

                return Ok(());
            }
            Equal => {
                // Duplicate
                // RFC, §2.3:
                // > In addition, an AU MUST NOT be repeated in other RTP packets; hence
                // > repetition of an AU is only possible when using a duplicate RTP packet.
                //
                // But: we can't received duplicates because they would have been rejected
                // by the base class or the jitterbuffer.
                unreachable!();
            }
        }

        // Upcoming AU is not then new head

        loop {
            let Some(next) = cur_node.next else {
                let new = Some(self.early_aus.insert(AuNode { au, next: None }));
                self.early_aus.get_mut(cur).unwrap().next = new;

                return Ok(());
            };

            let next_node = self.early_aus.get(next).unwrap();

            match au.index.try_cmp(next_node.au.index)? {
                Greater => (), // try next node
                Less => {
                    let new = self.early_aus.insert(AuNode {
                        au,
                        next: Some(next),
                    });
                    self.early_aus.get_mut(cur).unwrap().next = Some(new);

                    return Ok(());
                }
                Equal => {
                    // Duplicate
                    // RFC, §2.3:
                    // > In addition, an AU MUST NOT be repeated in other RTP packets; hence
                    // > repetition of an AU is only possible when using a duplicate RTP packet.
                    //
                    // But: we can't received duplicates because they would have been rejected
                    // by the base class or the jitterbuffer.
                    unreachable!();
                }
            }

            cur = next;
            cur_node = next_node;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4g::depay::SingleAuOrList;

    impl From<u32> for AccessUnit {
        fn from(index: u32) -> Self {
            AccessUnit {
                index: index.into(),
                ..Default::default()
            }
        }
    }

    #[test]
    fn simple_group_interleave() {
        // Tests the pattern illustrated in:
        // https://www.rfc-editor.org/rfc/rfc3640.html#appendix-A.3

        gst::init().unwrap();

        let mut deint_buf = DeinterleaveAuBuffer::default();
        assert!(deint_buf.early_aus.is_empty());
        assert!(deint_buf.expected_index.is_none());

        let mut outbuf = MaybeSingleAuOrList::default();
        assert!(outbuf.0.is_none());

        // ****
        // * P0. AUs with indices: 0, 3 & 6

        // Expected AU 0 so it is pushed to outbuf
        deint_buf.push_and_pop(0.into(), &mut outbuf).unwrap();

        assert!(deint_buf.early_aus.is_empty());
        assert_eq!(deint_buf.expected_index.unwrap(), 1);
        matches!(outbuf.0, Some(SingleAuOrList::Single(_)));

        // Expected AU 1 is missing when pushing AU 3 so it is buffered
        deint_buf.push_and_pop(3.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 1);
        assert_eq!(deint_buf.expected_index.unwrap(), 1);
        matches!(outbuf.0, Some(SingleAuOrList::Single(_)));

        // Expected AU 1 is missing when pushing AU 6 so it is buffered
        deint_buf.push_and_pop(6.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 2);
        assert_eq!(deint_buf.expected_index.unwrap(), 1);

        // End of the RTP packet
        matches!(outbuf.take(), Some(SingleAuOrList::Single(_)));
        assert!(outbuf.0.is_none());

        // ****
        // * P1. AUs with indices: 1, 4 & 7

        // Expected AU 1 so it is pushed to outbuf
        deint_buf.push_and_pop(1.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 2);
        assert_eq!(deint_buf.expected_index.unwrap(), 2);
        matches!(outbuf.0, Some(SingleAuOrList::Single(_)));

        // Expected AU 2 is missing when pushing AU 4 so it is buffered
        deint_buf.push_and_pop(4.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 3);
        assert_eq!(deint_buf.expected_index.unwrap(), 2);
        matches!(outbuf.0, Some(SingleAuOrList::Single(_)));

        // Expected AU 2 is missing when pushing AU 7 so it is buffered
        deint_buf.push_and_pop(7.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 4);
        assert_eq!(deint_buf.expected_index.unwrap(), 2);

        // End of the RTP packet
        matches!(outbuf.take(), Some(SingleAuOrList::Single(_)));
        assert!(outbuf.0.is_none());

        // ****
        // * P2. AUs with indices: 2, 5 & 8

        // Expected AU 2 so it is pushed to outbuf
        // and this also pops AUs 3 & 4
        // Remaining: 6 & 7
        deint_buf.push_and_pop(2.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 2);
        assert_eq!(deint_buf.expected_index.unwrap(), 5);
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.0 else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 3);

        // Expected AU 5 so it is pushed to outbuf
        // and this also pops AUs 6 & 7
        deint_buf.push_and_pop(5.into(), &mut outbuf).unwrap();

        assert!(deint_buf.early_aus.is_empty());
        assert_eq!(deint_buf.expected_index.unwrap(), 8);
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.0 else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 6);

        // Expected AU 8 so it is pushed to outbuf
        deint_buf.push_and_pop(8.into(), &mut outbuf).unwrap();

        assert!(deint_buf.early_aus.is_empty());
        assert_eq!(deint_buf.expected_index.unwrap(), 9);

        // End of the RTP packet
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.take() else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 7);
        assert!(outbuf.0.is_none());

        // ****
        // * P3. AUs with indices: 9, 12 & 15

        // Expected AU 9 so it is pushed to outbuf
        deint_buf.push_and_pop(9.into(), &mut outbuf).unwrap();

        assert!(deint_buf.early_aus.is_empty());
        assert_eq!(deint_buf.expected_index.unwrap(), 10);
        matches!(outbuf.0, Some(SingleAuOrList::Single(_)));

        // Expected AU 10 is missing when pushing AU 12 so it is buffered
        deint_buf.push_and_pop(12.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 1);
        assert_eq!(deint_buf.expected_index.unwrap(), 10);
        matches!(outbuf.0, Some(SingleAuOrList::Single(_)));

        // Expected AU 10 is missing when pushing AU 15 so it is buffered
        deint_buf.push_and_pop(15.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 2);
        assert_eq!(deint_buf.expected_index.unwrap(), 10);

        // End of the RTP packet
        matches!(outbuf.take(), Some(SingleAuOrList::Single(_)));
        assert!(outbuf.0.is_none());
    }

    #[test]
    fn more_subtle_group_interleave() {
        // Tests the pattern illustrated in:
        // https://www.rfc-editor.org/rfc/rfc3640.html#appendix-A.4

        gst::init().unwrap();

        let mut deint_buf = DeinterleaveAuBuffer::default();
        let mut outbuf = MaybeSingleAuOrList::default();

        // ****
        // * P0. AUs with indices: 0 & 5

        // Expected AU 0 so it is pushed to outbuf
        deint_buf.push_and_pop(0.into(), &mut outbuf).unwrap();

        assert!(deint_buf.early_aus.is_empty());
        assert_eq!(deint_buf.expected_index.unwrap(), 1);
        matches!(outbuf.0, Some(SingleAuOrList::Single(_)));

        // Expected AU 1 is missing when pushing AU 5 so it is buffered
        deint_buf.push_and_pop(5.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 1);
        assert_eq!(deint_buf.expected_index.unwrap(), 1);

        // End of the RTP packet
        matches!(outbuf.take(), Some(SingleAuOrList::Single(_)));
        assert!(outbuf.0.is_none());

        // ****
        // * P1. AUs with indices: 2 & 7

        // Expected AU 1 is missing when pushing AU 2 so it is buffered
        deint_buf.push_and_pop(2.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 2);
        assert_eq!(deint_buf.expected_index.unwrap(), 1);
        assert!(outbuf.0.is_none());

        // Expected AU 1 is missing when pushing AU 7 so it is buffered
        deint_buf.push_and_pop(7.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 3);
        assert_eq!(deint_buf.expected_index.unwrap(), 1);

        // End of the RTP packet
        assert!(outbuf.take().is_none());

        // ****
        // * P2. AUs with indices: 4 & 9

        // Expected AU 1 is missing when pushing AU 4 so it is buffered
        deint_buf.push_and_pop(4.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 4);
        assert_eq!(deint_buf.expected_index.unwrap(), 1);
        assert!(outbuf.0.is_none());

        // Expected AU 1 is missing when pushing AU 9 so it is buffered
        deint_buf.push_and_pop(9.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 5);
        assert_eq!(deint_buf.expected_index.unwrap(), 1);

        // End of the RTP packet
        assert!(outbuf.take().is_none());

        // ****
        // * P3. AUs with indices: 1 & 6

        // Expected AU 1 so it is pushed to outbuf
        // and this also pops AU 2
        // Remaining: 4, 5, 7 & 9
        deint_buf.push_and_pop(1.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 4);
        assert_eq!(deint_buf.expected_index.unwrap(), 3);
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.0 else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 2);

        // Expected AU 3 is missing when pushing AU 6 so it is buffered
        deint_buf.push_and_pop(6.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 5);
        assert_eq!(deint_buf.expected_index.unwrap(), 3);

        // End of the RTP packet
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.take() else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 2);
        assert!(outbuf.0.is_none());

        // ****
        // * P4. AUs with indices: 3 & 8

        // Expected AU 3 so it is pushed to outbuf
        // and this also pops AU 4, 5, 6 & 7
        // Remaining: 9
        deint_buf.push_and_pop(3.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 1);
        assert_eq!(deint_buf.expected_index.unwrap(), 8);
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.0 else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 5);

        // Expected AU 8 so it is pushed to outbuf
        // and this also pops AU 9
        deint_buf.push_and_pop(8.into(), &mut outbuf).unwrap();

        assert!(deint_buf.early_aus.is_empty());
        assert_eq!(deint_buf.expected_index.unwrap(), 10);

        // End of the RTP packet
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.take() else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 7);
        assert!(outbuf.0.is_none());

        // ****
        // * P5. AUs with indices: 10 & 15

        // Expected AU 10 so it is pushed to outbuf
        deint_buf.push_and_pop(10.into(), &mut outbuf).unwrap();

        assert!(deint_buf.early_aus.is_empty());
        assert_eq!(deint_buf.expected_index.unwrap(), 11);
        matches!(outbuf.0, Some(SingleAuOrList::Single(_)));

        // Expected AU 11 is missing when pushing AU 15 so it is buffered
        deint_buf.push_and_pop(15.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 1);
        assert_eq!(deint_buf.expected_index.unwrap(), 11);

        // End of the RTP packet
        matches!(outbuf.take(), Some(SingleAuOrList::Single(_)));
        assert!(outbuf.0.is_none());
    }

    #[test]
    fn continuous_interleave() {
        // Tests the pattern illustrated in:
        // https://www.rfc-editor.org/rfc/rfc3640.html#appendix-A.5

        gst::init().unwrap();

        let mut deint_buf = DeinterleaveAuBuffer::default();
        let mut outbuf = MaybeSingleAuOrList::default();

        // ****
        // * P0. AUs with index: 0

        // Expected AU 0 so it is pushed to outbuf
        deint_buf.push_and_pop(0.into(), &mut outbuf).unwrap();

        assert!(deint_buf.early_aus.is_empty());
        assert_eq!(deint_buf.expected_index.unwrap(), 1);

        // End of the RTP packet
        matches!(outbuf.take(), Some(SingleAuOrList::Single(_)));
        assert!(outbuf.0.is_none());

        // ****
        // * P1. AUs with indices: 1 & 4

        // Expected AU 0 so it is pushed to outbuf
        deint_buf.push_and_pop(1.into(), &mut outbuf).unwrap();

        assert!(deint_buf.early_aus.is_empty());
        assert_eq!(deint_buf.expected_index.unwrap(), 2);
        matches!(outbuf.0, Some(SingleAuOrList::Single(_)));

        // Expected AU 2 is missing when pushing AU 4 so it is buffered
        deint_buf.push_and_pop(4.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 1);
        assert_eq!(deint_buf.expected_index.unwrap(), 2);

        // End of the RTP packet
        matches!(outbuf.take(), Some(SingleAuOrList::Single(_)));
        assert!(outbuf.take().is_none());

        // ****
        // * P2. AUs with indices: 2, 5 & 8

        // Expected AU 2 so it is pushed to outbuf
        deint_buf.push_and_pop(2.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 1);
        assert_eq!(deint_buf.expected_index.unwrap(), 3);
        matches!(outbuf.0, Some(SingleAuOrList::Single(_)));

        // Expected AU 3 is missing when pushing AU 5 so it is buffered
        deint_buf.push_and_pop(5.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 2);
        assert_eq!(deint_buf.expected_index.unwrap(), 3);
        matches!(outbuf.0, Some(SingleAuOrList::Single(_)));

        // Expected AU 3 is missing when pushing AU 8 so it is buffered
        deint_buf.push_and_pop(8.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 3);
        assert_eq!(deint_buf.expected_index.unwrap(), 3);

        // End of the RTP packet
        matches!(outbuf.take(), Some(SingleAuOrList::Single(_)));
        assert!(outbuf.take().is_none());

        // ****
        // * P3. AUs with indices: 3, 6, 9 & 12

        // Expected AU 3 so it is pushed to outbuf
        // and this also pops AU 4 & 5
        // Remaining: 8
        deint_buf.push_and_pop(3.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 1);
        assert_eq!(deint_buf.expected_index.unwrap(), 6);
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.0 else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 3);

        // Expected AU 6 so it is pushed to outbuf
        deint_buf.push_and_pop(6.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 1);
        assert_eq!(deint_buf.expected_index.unwrap(), 7);
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.0 else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 4);

        // Expected AU 7 is missing when pushing AU 9 so it is buffered
        deint_buf.push_and_pop(9.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 2);
        assert_eq!(deint_buf.expected_index.unwrap(), 7);
        matches!(outbuf.0, Some(SingleAuOrList::List(_)));

        // Expected AU 7 is missing when pushing AU 12 so it is buffered
        deint_buf.push_and_pop(12.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 3);
        assert_eq!(deint_buf.expected_index.unwrap(), 7);

        // End of the RTP packet
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.take() else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 4);
        assert!(outbuf.0.is_none());

        // ****
        // * P4. AUs with indices: 7, 10, 13 & 16

        // Expected AU 7 so it is pushed to outbuf
        // and this also pops AU 8 & 9
        // Remaining: 12
        deint_buf.push_and_pop(7.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 1);
        assert_eq!(deint_buf.expected_index.unwrap(), 10);
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.0 else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 3);

        // Expected AU 10 so it is pushed to outbuf
        deint_buf.push_and_pop(10.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 1);
        assert_eq!(deint_buf.expected_index.unwrap(), 11);
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.0 else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 4);

        // Expected AU 11 is missing when pushing AU 13 so it is buffered
        deint_buf.push_and_pop(13.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 2);
        assert_eq!(deint_buf.expected_index.unwrap(), 11);
        matches!(outbuf.0, Some(SingleAuOrList::List(_)));

        // Expected AU 11 is missing when pushing AU 16 so it is buffered
        deint_buf.push_and_pop(16.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 3);
        assert_eq!(deint_buf.expected_index.unwrap(), 11);

        // End of the RTP packet
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.take() else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 4);
        assert!(outbuf.0.is_none());

        // ****
        // * P5. AUs with indices: 11, 14, 17 & 20

        // Expected AU 11 so it is pushed to outbuf
        // and this also pops AU 12 & 13
        // Remaining: 16
        deint_buf.push_and_pop(11.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 1);
        assert_eq!(deint_buf.expected_index.unwrap(), 14);
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.0 else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 3);

        // Expected AU 14 so it is pushed to outbuf
        deint_buf.push_and_pop(14.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 1);
        assert_eq!(deint_buf.expected_index.unwrap(), 15);
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.0 else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 4);

        // Expected AU 15 is missing when pushing AU 17 so it is buffered
        deint_buf.push_and_pop(17.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 2);
        assert_eq!(deint_buf.expected_index.unwrap(), 15);
        matches!(outbuf.0, Some(SingleAuOrList::List(_)));

        // Expected AU 15 is missing when pushing AU 20 so it is buffered
        deint_buf.push_and_pop(20.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 3);
        assert_eq!(deint_buf.expected_index.unwrap(), 15);

        // End of the RTP packet
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.take() else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 4);
        assert!(outbuf.0.is_none());

        // ****
        // * P6. AUs with indices: 15 & 18

        // Expected AU 15 so it is pushed to outbuf
        // and this also pops AU 16 & 17
        // Remaining: 20
        deint_buf.push_and_pop(15.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 1);
        assert_eq!(deint_buf.expected_index.unwrap(), 18);
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.0 else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 3);

        // Expected AU 18 so it is pushed to outbuf
        deint_buf.push_and_pop(18.into(), &mut outbuf).unwrap();

        assert_eq!(deint_buf.early_aus.len(), 1);
        assert_eq!(deint_buf.expected_index.unwrap(), 19);

        // End of the RTP packet
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.take() else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 4);
        assert!(outbuf.0.is_none());

        // ****
        // * P7. AUs with index: 19
        deint_buf.push_and_pop(19.into(), &mut outbuf).unwrap();

        // Expected AU 19 so it is pushed to outbuf
        // and this also pops AU 20
        assert!(deint_buf.early_aus.is_empty());
        assert_eq!(deint_buf.expected_index.unwrap(), 21);

        // End of the RTP packet
        let Some(SingleAuOrList::List(ref buflist)) = outbuf.take() else {
            panic!("Expecting a List");
        };
        assert_eq!(buflist.len(), 2);
        assert!(outbuf.0.is_none());
    }
}
