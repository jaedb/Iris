use anyhow::Context;
use bitstream_io::{BigEndian, BitRead, BitReader, ByteRead, ByteReader};

use std::io::Cursor;

use super::{AccessUnit, Mpeg4GenericDepayError};
use crate::mp4g::{AccessUnitIndex, AuHeader, AuHeaderContext, ModeConfig, RtpTimestamp};

/// The reference Packet to compute constant duration when applicable.
#[derive(Debug, Default)]
struct ConstantDurationProbation {
    ts: RtpTimestamp,
    frames: u32,
}

impl From<RtpTimestamp> for ConstantDurationProbation {
    fn from(ts: RtpTimestamp) -> Self {
        ConstantDurationProbation { ts, frames: 0 }
    }
}

/// MPEG-4 generic Payload: https://www.rfc-editor.org/rfc/rfc3640.html#section-2.11
///
/// +---------+-----------+-----------+---------------+
/// | RTP     | AU Header | Auxiliary | Access Unit   |
/// | Header  | Section   | Section   | Data Section  |
/// +---------+-----------+-----------+---------------+
///
/// .         <----------RTP Packet Payload----------->
#[derive(Debug, Default)]
pub struct PayloadParser {
    config: ModeConfig,
    const_dur_probation: Option<ConstantDurationProbation>,
    // Constant duration as provided by the config
    // or determined while parsing the payloads
    constant_duration: Option<u32>,
}

impl PayloadParser {
    #[allow(dead_code)]
    pub fn new_for(config: ModeConfig) -> Self {
        PayloadParser {
            constant_duration: config.constant_duration(),
            config,
            ..Default::default()
        }
    }

    pub fn set_config(&mut self, config: ModeConfig) {
        self.config = config;
        self.reset();
    }

    pub fn reset(&mut self) {
        self.constant_duration = self.config.constant_duration();
        self.const_dur_probation = None;
    }

    pub fn parse<'a>(
        &'a mut self,
        payload: &'a [u8],
        ext_seqnum: u64,
        packet_ts: RtpTimestamp,
    ) -> anyhow::Result<AccessUnitIter<'a>> {
        use Mpeg4GenericDepayError::*;

        let mut headers_len = 0;
        let mut headers = None;
        let mut data_offset = 0;

        let mut r = ByteReader::endian(payload, BigEndian);

        if self.config.has_header_section() {
            // AU Header section: https://www.rfc-editor.org/rfc/rfc3640.html#section-3.2.1
            //
            // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+- .. -+-+-+-+-+-+-+-+-+-+
            // |AU-headers-length|AU-header|AU-header|      |AU-header|padding|
            // |                 |   (1)   |   (2)   |      |   (n) * | bits  |
            // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+- .. -+-+-+-+-+-+-+-+-+-+

            // This is expressed in bits
            headers_len = r.read::<u16>().context("AU-headers-length")?;

            // Up to 7 bits of padding
            let headers_len_bytes = (headers_len as usize).div_ceil(8);

            data_offset = 2 + headers_len_bytes;
            if data_offset > payload.len() {
                Err(AuHeaderSectionTooLarge {
                    expected_end: data_offset,
                    total: payload.len(),
                })?;
            }

            r.skip(headers_len_bytes as u32)
                .expect("availability checked above");

            headers = Some(&payload[2..data_offset]);
        } else if self.constant_duration.is_none() {
            // No headers and non-constant duration

            // § 3.2.3.2:
            // > When transmitting Access Units of variable duration, then the
            // > "constantDuration" parameter MUST NOT be present [...]
            // > the CTS-delta MUST be coded in the AU header for each non-first AU
            // > in the RTP packet

            Err(NonConstantDurationNoAuHeaders { ext_seqnum })?;
        }

        if self.config.has_auxiliary_section() {
            // Move the AU reader after the Auxiliary Section
            //
            // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+- .. -+-+-+-+-+-+-+-+-+
            // | auxiliary-data-size   | auxiliary-data       |padding bits |
            // +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+- .. -+-+-+-+-+-+-+-+-+

            // This is expressed in bits
            let aux_len = r.read::<u16>().context("auxiliary-data-size")?;

            // Up to 7 bits of padding
            let aux_len_bytes = (aux_len as usize).div_ceil(8);

            data_offset += 2 + aux_len_bytes;
            if data_offset > payload.len() {
                Err(AuAuxiliarySectionTooLarge {
                    expected_end: data_offset,
                    total: payload.len(),
                })?;
            }
        }

        if data_offset >= payload.len() {
            Err(EmptyAuData)?;
        }

        Ok(AccessUnitIter {
            parser: self,
            ext_seqnum,
            packet_ts,
            prev_index: None,
            headers_r: headers.map(|h| BitReader::endian(Cursor::new(h), BigEndian)),
            headers_len,
            data: &payload[data_offset..],
            cur: 0,
        })
    }
}

#[derive(Debug)]
pub struct AccessUnitIter<'a> {
    parser: &'a mut PayloadParser,
    ext_seqnum: u64,
    packet_ts: RtpTimestamp,
    prev_index: Option<AccessUnitIndex>,
    headers_r: Option<BitReader<Cursor<&'a [u8]>, BigEndian>>,
    headers_len: u16,
    data: &'a [u8],
    cur: u32,
}

impl Iterator for AccessUnitIter<'_> {
    type Item = anyhow::Result<AccessUnit>;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.next_priv();
        if let Some(Err(_)) = res.as_ref() {
            self.parser.reset();
        }

        res
    }
}

impl AccessUnitIter<'_> {
    fn next_priv(&mut self) -> Option<anyhow::Result<AccessUnit>> {
        use Mpeg4GenericDepayError::*;

        let mut cts_delta = None;
        let mut duration = None;
        let header = if let Some(r) = self.headers_r.as_mut() {
            let pos = r.position_in_bits().unwrap() as u16;
            if pos >= self.headers_len {
                // No more AUs
                return None;
            }

            let ctx = AuHeaderContext {
                config: &self.parser.config,
                prev_index: self.prev_index,
            };

            let header = match r.parse_with::<AuHeader>(&ctx) {
                Ok(header) => header,
                Err(err) => {
                    return Some(
                        Err(err).with_context(|| format!("AuHeader in packet {}", self.ext_seqnum)),
                    )
                }
            };

            if self.prev_index.is_none() {
                // First AU header of the packet
                if header.index.is_zero() {
                    if self.parser.constant_duration.is_none() {
                        // > In the absence of the constantDuration parameter
                        // > receivers MUST conclude that the AUs have constant duration
                        // > if the AU-index is zero in two consecutive RTP packets.

                        if let Some(cd_prob) = self.parser.const_dur_probation.take() {
                            let dur = *(self.packet_ts - cd_prob.ts) / cd_prob.frames;
                            self.parser.constant_duration = Some(dur);
                        } else {
                            // Keep first packet for constant duration probation:
                            self.parser.const_dur_probation = Some(self.packet_ts.into());
                        }
                    }
                } else if self.parser.constant_duration.is_some() {
                    // § 3.2.3.2:
                    // > when transmitting Access Units of constant duration, the AU-Index,
                    // > if present, MUST be set to the value 0
                    return Some(Err(ConstantDurationAuNonZeroIndex {
                        index: header.index,
                        ext_seqnum: self.ext_seqnum,
                    }
                    .into()));
                } else if self.parser.const_dur_probation.is_some() {
                    // Constant duration was in probation but index is not zero
                    // => not switching to constant duration yet
                    self.parser.const_dur_probation = None;
                }
            }
            if let Some(delta) = header.cts_delta {
                cts_delta = Some(delta);
            } else if let Some(constant_duration) = self.parser.constant_duration {
                // § 3.2.3.2:
                // > If the "constantDuration" parameter is present, the receiver can
                // > reconstruct the original Access Unit timing based solely on the RTP
                // > timestamp and AU-Index-delta.
                cts_delta = Some((*header.index * constant_duration) as i32);
                duration = Some(constant_duration);
            } else if self.prev_index.is_some() && self.parser.const_dur_probation.is_none() {
                // Non-constant duration, no CTS-delta, not first header,
                // and not constant duration probation in progress

                // § 3.2.3.2:
                // > When transmitting Access Units of variable duration, then the
                // > "constantDuration" parameter MUST NOT be present [...]
                // > the CTS-delta MUST be coded in the AU header for each non-first AU
                // > in the RTP packet

                return Some(Err(NonConstantDurationAuNoCtsDelta {
                    index: header.index,
                    ext_seqnum: self.ext_seqnum,
                }
                .into()));
            } else if self.prev_index.is_none() {
                // First AU but unknown duration
                cts_delta = Some(0);
            }

            if header.size.is_none() && self.parser.config.constant_size == 0 {
                // § 3.2.3:
                // > The absence of both AU-size in the AU-header and the constantSize
                // > MIME format parameter indicates the carriage of a single AU
                // > (fragment), i.e., that a single Access Unit (fragment) is transported
                // > in each RTP packet for that stream

                let pos = r.position_in_bits().unwrap() as u16;
                if pos < self.headers_len {
                    // More headers to read
                    return Some(Err(MultipleAusUnknownSize.into()));
                }
            }

            if self.data.is_empty() {
                // We have exhausted the data section, but there are more headers
                return Some(Err(NoMoreAuDataLeft {
                    index: header.index,
                }
                .into()));
            }

            self.prev_index = Some(header.index);

            header
        } else {
            // No header section

            if self.data.is_empty() {
                // We have exhausted the data section
                return None;
            }

            let constant_duration = self
                .parser
                .constant_duration
                .expect("checked in PayloadParser::parse");

            cts_delta = Some((self.cur * constant_duration) as i32);
            duration = Some(constant_duration);

            AuHeader::new_with(self.cur)
        };

        let mut is_fragment = false;

        // § 3.2.3
        // > If the AU size is variable, then the
        // > size of each AU MUST be indicated in the AU-size field of the
        // > corresponding AU-header. However, if the AU size is constant for a
        // > stream, this mechanism SHOULD NOT be used; instead, the fixed size
        // > SHOULD be signaled by the MIME format parameter "constantSize"

        let au_size = if self.parser.config.constant_size > 0 {
            self.parser.config.constant_size as usize
        } else if let Some(size) = header.size {
            size as usize
        } else {
            // Unknown size
            // Note: MultipleAusUnknownSize case checked above

            self.data.len()
        };

        let data = if au_size <= self.data.len() {
            let data = self.data[..au_size].to_owned();

            // Update self.data for next AU
            self.data = &self.data[au_size..];

            data
        } else {
            // The len of the AU can exceed the AU data len in case of a framgment
            if self.cur > 0 {
                return Some(Err(MultipleAusGreaterSizeThanAuData {
                    au_size,
                    au_data_size: self.data.len(),
                }
                .into()));
            }

            is_fragment = true;

            let data = self.data[..].to_owned();
            self.data = &[];

            data
        };

        self.cur += 1;

        if let Some(ref mut cd_prob) = self.parser.const_dur_probation {
            cd_prob.frames = self.cur;
        }

        Some(Ok(AccessUnit {
            ext_seqnum: self.ext_seqnum,
            is_fragment,
            size: header.size,
            index: header.index,
            cts_delta,
            dts_delta: header.dts_delta,
            duration,
            maybe_random_access: header.maybe_random_access,
            is_interleaved: header.is_interleaved,
            data,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Mpeg4GenericDepayError::*;

    const TS0: RtpTimestamp = RtpTimestamp::ZERO;
    const TS21: RtpTimestamp = RtpTimestamp::new(21);

    #[test]
    fn no_headers_one_au() {
        const CONSTANT_DURATION: u32 = 3;
        let mut parser = PayloadParser::new_for(ModeConfig {
            constant_duration: CONSTANT_DURATION,
            ..Default::default()
        });

        let payload = &[0, 1, 2, 3];
        let mut iter = parser.parse(payload, 0, TS0).unwrap();
        assert_eq!(iter.data.len(), 4);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.index, 0);
        assert!(!au.is_interleaved);
        assert!(!au.is_fragment); // <==
        assert_eq!(au.cts_delta, Some(0i32));
        assert!(au.dts_delta.is_none());
        assert_eq!(au.duration, Some(CONSTANT_DURATION));
        assert!(au.maybe_random_access.is_none());
        assert_eq!(au.data, payload);

        assert!(iter.next().is_none());
    }

    #[test]
    fn no_headers_one_au_fragmented() {
        const CONSTANT_DURATION: u32 = 3;
        let mut parser = PayloadParser::new_for(ModeConfig {
            constant_size: 6,
            constant_duration: 3,
            ..Default::default()
        });

        let payload = &[0, 1, 2, 3];
        let mut iter = parser.parse(payload, 42, TS0).unwrap();
        assert_eq!(iter.data.len(), 4);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.ext_seqnum, 42);
        assert_eq!(au.index, 0);
        assert!(!au.is_interleaved);
        assert!(au.is_fragment); // <==
        assert_eq!(au.duration, Some(CONSTANT_DURATION));
        assert_eq!(au.data, payload);

        assert!(iter.next().is_none());
    }

    #[test]
    fn no_headers_two_aus() {
        const CONSTANT_DURATION: u32 = 5;
        let mut parser = PayloadParser::new_for(ModeConfig {
            constant_size: 2,
            constant_duration: CONSTANT_DURATION,
            ..Default::default()
        });

        let payload = &[0, 1, 2, 3];
        let mut iter = parser.parse(payload, 42, TS21).unwrap();

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.ext_seqnum, 42);
        assert_eq!(au.index, 0);
        assert!(!au.is_interleaved);
        assert_eq!(au.cts_delta, Some(0i32));
        assert!(au.dts_delta.is_none());
        assert_eq!(au.duration, Some(CONSTANT_DURATION));
        assert!(!au.is_fragment);
        assert_eq!(au.data, payload[..2]);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.ext_seqnum, 42);
        assert_eq!(au.index, 1);
        assert!(!au.is_interleaved);
        assert_eq!(au.cts_delta, Some(CONSTANT_DURATION as i32));
        assert!(au.dts_delta.is_none());
        assert_eq!(au.duration, Some(CONSTANT_DURATION));
        assert!(!au.is_fragment);
        assert_eq!(au.data, payload[2..][..2]);

        assert!(iter.next().is_none());
    }

    #[test]
    fn no_headers_empty_au_data() {
        let mut parser = PayloadParser::new_for(ModeConfig {
            constant_size: 2,
            constant_duration: 2,
            ..Default::default()
        });

        let payload = &[];
        let err = parser
            .parse(payload, 0, TS0)
            .unwrap_err()
            .downcast::<Mpeg4GenericDepayError>()
            .unwrap();
        assert_eq!(err, EmptyAuData);
    }

    #[test]
    fn no_headers_two_aus_one_fragment() {
        let mut parser = PayloadParser::new_for(ModeConfig {
            constant_size: 3,
            constant_duration: 2,
            ..Default::default()
        });

        let payload = &[0, 1, 2, 3];
        let mut iter = parser.parse(payload, 0, TS0).unwrap();

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.index, 0);
        assert!(!au.is_fragment);
        assert_eq!(au.data, payload[..3]);

        let err = iter
            .next()
            .unwrap()
            .unwrap_err()
            .downcast::<Mpeg4GenericDepayError>()
            .unwrap();
        assert_eq!(
            err,
            MultipleAusGreaterSizeThanAuData {
                au_size: 3,
                au_data_size: 1
            }
        );
    }

    #[test]
    fn header_one_au() {
        let mut parser = PayloadParser::new_for(ModeConfig {
            size_len: 2,
            ..Default::default()
        });

        // Header section size: 2 bytes.
        // Header:
        // * 2 bits: data size (3 here => b11.._....)
        let payload = &[0x00, 0x02, 0xc0, 0x01, 0x02, 0x03];
        let mut iter = parser.parse(payload, 42, TS0).unwrap();
        assert_eq!(iter.data.len(), 3);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.ext_seqnum, 42);
        assert_eq!(au.index, 0);
        assert!(!au.is_interleaved);
        assert!(!au.is_fragment); // <==
        assert_eq!(au.cts_delta, Some(0i32));
        assert!(au.dts_delta.is_none());
        assert!(au.duration.is_none());
        assert!(au.maybe_random_access.is_none());
        assert_eq!(au.data, payload[3..][..3]);

        assert!(iter.next().is_none());
    }

    #[test]
    fn header_one_au_cts() {
        let mut parser = PayloadParser::new_for(ModeConfig {
            cts_delta_len: 2,
            ..Default::default()
        });

        // Header section size: 2 bytes.
        // Header:
        // * 1 bits: CTS flag not set because 1st header (0 here => b0...)
        // * following 2 bits for CTS delta not present
        let payload = &[0x00, 0x01, 0x00, 0x01, 0x02];
        let mut iter = parser.parse(payload, 0, TS21).unwrap();
        assert_eq!(iter.data.len(), 2);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.ext_seqnum, 0);
        assert!(!au.is_interleaved);
        assert_eq!(au.cts_delta, Some(0i32));
        assert!(au.dts_delta.is_none());
        assert!(au.duration.is_none());
        assert!(au.maybe_random_access.is_none());
        assert_eq!(au.data, payload[3..][..2]);

        assert!(iter.next().is_none());
    }

    #[test]
    fn header_one_au_cts_set() {
        use crate::mp4g::header::AuHeaderError::{self, *};

        let mut parser = PayloadParser::new_for(ModeConfig {
            cts_delta_len: 2,
            ..Default::default()
        });

        // Header section size: 2 bytes.
        // Header:
        // * 3 bits: CTS flag + CTS delta (-1 here => b111.)
        let payload = &[0x00, 0x03, 0xe0, 0x01, 0x02];
        let mut iter = parser.parse(payload, 42, TS21).unwrap();
        assert_eq!(iter.data.len(), 2);

        let err = iter
            .next()
            .unwrap()
            .unwrap_err()
            .downcast::<AuHeaderError>()
            .unwrap();
        assert_eq!(err, CtsFlagSetInFirstAuHeader(AccessUnitIndex::ZERO));
    }

    #[test]
    fn header_one_au_random_access() {
        let mut parser = PayloadParser::new_for(ModeConfig {
            random_access_indication: true,
            ..Default::default()
        });

        // Header section size: 2 bytes.
        // Header:
        // * 1 bit: random access flag (1 here => b1...)
        let payload = &[0x00, 0x01, 0x80, 0x01, 0x02];
        let mut iter = parser.parse(payload, 0, TS21).unwrap();
        assert_eq!(iter.data.len(), 2);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.ext_seqnum, 0);
        assert!(!au.is_interleaved);
        assert_eq!(au.maybe_random_access, Some(true));
        assert_eq!(au.data, payload[3..][..2]);

        assert!(iter.next().is_none());
    }

    #[test]
    fn header_one_au_random_access_stream_state() {
        let mut parser = PayloadParser::new_for(ModeConfig {
            random_access_indication: true,
            stream_state_indication: 2,
            ..Default::default()
        });

        // Header section size: 2 bytes.
        // Header:
        // * 1 bit: random access flag (1 here => b1...)
        // * 2 bits: stream state (3 here => b.11._....)
        let payload = &[0x00, 0x03, 0xe0, 0x01, 0x02];
        let mut iter = parser.parse(payload, 0, TS0).unwrap();
        assert_eq!(iter.data.len(), 2);

        let au = iter.next().unwrap().unwrap();
        assert!(!au.is_interleaved);
        assert_eq!(au.maybe_random_access, Some(true));
        assert_eq!(au.data, payload[3..][..2]);

        assert!(iter.next().is_none());
    }

    #[test]
    fn header_one_au_fragemented() {
        let mut parser = PayloadParser::new_for(ModeConfig {
            size_len: 2,
            ..Default::default()
        });

        // Header section size: 2 bytes.
        // Header:
        // * 2 bits: data size (3 here => b11.._....)
        let payload = &[0x00, 0x02, 0xc0, 0x01, 0x02];
        let mut iter = parser.parse(payload, 0, TS0).unwrap();
        assert_eq!(iter.data.len(), 2);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.index, 0);
        assert!(!au.is_interleaved);
        assert!(au.is_fragment); // <==
        assert_eq!(au.data, payload[3..][..2]);

        assert!(iter.next().is_none());
    }

    #[test]
    fn header_two_aus() {
        const CONSTANT_DURATION: u32 = 5;
        let mut parser = PayloadParser::new_for(ModeConfig {
            size_len: 2,
            constant_duration: CONSTANT_DURATION,
            ..Default::default()
        });

        // Header section size: 2 bytes.
        // Each AU-header:
        // * 2 bits: data size (AU1: 2 => b10.._...., AU2: 3 => b..11_....)
        let payload = &[0x00, 0x04, 0xb0, 0x01, 0x02, 0x03, 0x04, 0x05];
        let mut iter = parser.parse(payload, 42, TS0).unwrap();
        assert_eq!(iter.data.len(), 5);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.ext_seqnum, 42);
        assert_eq!(au.index, 0);
        assert!(!au.is_interleaved);
        assert_eq!(au.cts_delta, Some(0i32));
        assert!(au.dts_delta.is_none());
        assert_eq!(au.duration, Some(CONSTANT_DURATION));
        assert!(!au.is_fragment);
        assert_eq!(au.data, payload[3..][..2]);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.ext_seqnum, 42);
        assert_eq!(au.index, 1);
        assert_eq!(au.cts_delta, Some(CONSTANT_DURATION as i32));
        assert!(au.dts_delta.is_none());
        assert_eq!(au.duration, Some(CONSTANT_DURATION));
        assert!(!au.is_interleaved);
        assert!(!au.is_fragment);
        assert_eq!(au.data, payload[5..][..3]);

        assert!(iter.next().is_none());
    }

    #[test]
    fn header_two_aus_const_duration_probation() {
        const CONSTANT_DURATION: u32 = 2;

        let mut parser = PayloadParser::new_for(ModeConfig {
            size_len: 2,
            ..Default::default()
        });

        assert!(parser.constant_duration.is_none());
        assert!(parser.const_dur_probation.is_none());

        // Header section size: 2 bytes.
        // Each AU-header:
        // * 2 bits: data size (AU1: 2 => b10.._...., AU2: 3 => b..11_....)
        let payload = &[0x00, 0x04, 0xb0, 0x01, 0x02, 0x03, 0x04, 0x05];
        let mut iter = parser.parse(payload, 42, TS0).unwrap();

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.index, 0);
        assert_eq!(au.cts_delta, Some(0i32));
        assert!(au.dts_delta.is_none());
        assert!(au.duration.is_none());
        assert_eq!(au.data, payload[3..][..2]);

        let au = iter.next().unwrap().unwrap();
        assert!(au.cts_delta.is_none());
        assert!(au.dts_delta.is_none());
        assert!(au.duration.is_none());
        assert_eq!(au.data, payload[5..][..3]);

        assert!(iter.next().is_none());

        assert!(parser.constant_duration.is_none());
        assert!(parser.const_dur_probation.is_some());

        let ts4 = TS0 + 2 * CONSTANT_DURATION;

        // Header section size: 2 bytes.
        // Each AU-header:
        // * 2 bits: data size (AU1: 2 => b10.._...., AU2: 3 => b..11_....)
        let payload = &[0x00, 0x04, 0xb0, 0x01, 0x02, 0x03, 0x04, 0x05];
        let mut iter = parser.parse(payload, 42, ts4).unwrap();

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.index, 0);
        assert_eq!(au.cts_delta, Some(0i32));
        assert!(au.dts_delta.is_none());
        assert_eq!(au.duration, Some(CONSTANT_DURATION));
        assert_eq!(au.data, payload[3..][..2]);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.cts_delta, Some(CONSTANT_DURATION as i32));
        assert!(au.dts_delta.is_none());
        assert_eq!(au.duration, Some(CONSTANT_DURATION));
        assert_eq!(au.data, payload[5..][..3]);

        assert!(iter.next().is_none());

        assert_eq!(parser.constant_duration, Some(CONSTANT_DURATION));
        assert!(parser.const_dur_probation.is_none());
    }

    #[test]
    fn header_two_au_cts() {
        let mut parser = PayloadParser::new_for(ModeConfig {
            size_len: 2,
            cts_delta_len: 2,
            ..Default::default()
        });

        // Header section size: 2 bytes.
        // Header:
        // * Header 1
        //   - 2 bits: len 2 => b10..
        //   - 1 bit: CTS flag not set because 1st header => b..0.
        // * Header 2
        //   - 2 bits: len 3 => b...1_1...
        //   - 3 bits: CTS flag + CTS delta +1 in 2's comp => b...._.101)
        let payload = &[0x00, 0x08, 0x9d, 0x01, 0x02, 0x03, 0x04, 0x05];
        let mut iter = parser.parse(payload, 42, TS21).unwrap();
        assert_eq!(iter.data.len(), 5);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.ext_seqnum, 42);
        assert_eq!(au.index, 0);
        assert!(!au.is_interleaved);
        assert_eq!(au.cts_delta, Some(0i32));
        assert!(au.dts_delta.is_none());
        assert!(au.duration.is_none());
        assert!(!au.is_fragment);
        assert_eq!(au.data, payload[3..][..2]);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.ext_seqnum, 42);
        assert_eq!(au.index, 1);
        assert!(!au.is_interleaved);
        assert!(!au.is_fragment);
        assert_eq!(au.cts_delta, Some(1i32));
        assert!(au.dts_delta.is_none());
        assert!(au.duration.is_none());
        assert_eq!(au.data, payload[5..][..3]);

        assert!(iter.next().is_none());
    }

    #[test]
    fn header_three_aus_delta_index() {
        const CONSTANT_DURATION: u32 = 5;
        let mut parser = PayloadParser::new_for(ModeConfig {
            size_len: 2,
            constant_duration: CONSTANT_DURATION,
            index_delta_len: 1,
            ..Default::default()
        });

        // Header section size: 2 bytes.
        // Each AU-header: 2 bits: data size
        // For headers 2 & 3: 1 bits: AU-index-delta
        //
        // - AU1: size 2 => b10.._....
        // - AU2: size 3, delta 0 => b..11_0...
        // - AU3: size 1, delta 1 => b...._.011
        let payload = &[0x00, 0x07, 0xb3, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
        let mut iter = parser.parse(payload, 0, TS0).unwrap();
        assert_eq!(iter.data.len(), 6);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.index, 0);
        assert!(!au.is_interleaved);
        assert!(!au.is_fragment);
        assert_eq!(au.cts_delta, Some(0i32));
        assert!(au.dts_delta.is_none());
        assert_eq!(au.duration, Some(CONSTANT_DURATION));
        assert_eq!(au.data, payload[3..][..2]);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.index, 1);
        assert!(!au.is_interleaved);
        assert!(!au.is_fragment);
        assert_eq!(au.cts_delta, Some((*au.index * CONSTANT_DURATION) as i32));
        assert!(au.dts_delta.is_none());
        assert_eq!(au.duration, Some(CONSTANT_DURATION));
        assert_eq!(au.data, payload[5..][..3]);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.index, 3);
        assert!(au.is_interleaved);
        assert!(!au.is_fragment);
        assert_eq!(au.cts_delta, Some((*au.index * CONSTANT_DURATION) as i32));
        assert!(au.dts_delta.is_none());
        assert_eq!(au.duration, Some(CONSTANT_DURATION));
        assert_eq!(au.data, payload[8..][..1]);

        assert!(iter.next().is_none());
    }

    #[test]
    fn header_three_au_cts() {
        let mut parser = PayloadParser::new_for(ModeConfig {
            size_len: 2,
            cts_delta_len: 3,
            dts_delta_len: 3,
            ..Default::default()
        });

        // Header section size: 2 bytes.
        // Header:
        // * Header 1
        //   - 2 bits: len 2 => b10..
        //   - 1 bit: CTS flag not set because 1st header => b..0.
        //   - 4 bits: DTS flag + DTS delta -2 in 2's comp => b...1_110.)
        // * Header 2
        //   - 2 bits: len 3 => b...._...1 1..._....
        //   - 4 bits: CTS flag + CTS delta +1 in 2's comp => b.100_1...)
        //   - 4 bit: DTS flag + DTS delta 0 (i.e. same as CTS) => b...._.100 0..._....)
        // * Header 3
        //   - 2 bits: len 1 => b.01._....
        //   - 4 bits: CTS flag + CTS delta +2 in 2's comp => b...1_010.)
        //   - 4 bits: DTS flag + DTS delta -2 in 2's comp => b...._...1 110._....)
        let payload = &[
            0x00, 0x1b, 0x9d, 0xcc, 0x35, 0xc0, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06,
        ];
        let mut iter = parser.parse(payload, 0, TS21).unwrap();
        assert_eq!(iter.data.len(), 6);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.ext_seqnum, 0);
        assert_eq!(au.index, 0);
        assert!(!au.is_interleaved);
        assert!(!au.is_fragment);
        assert_eq!(au.cts_delta, Some(0i32));
        assert_eq!(au.dts_delta, Some(-2i32));
        assert_eq!(au.data, payload[6..][..2]);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.ext_seqnum, 0);
        assert_eq!(au.index, 1);
        assert!(!au.is_interleaved);
        assert!(!au.is_fragment);
        assert_eq!(au.cts_delta, Some(1i32));
        assert_eq!(au.dts_delta, Some(0i32));
        assert_eq!(au.data, payload[8..][..3]);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.ext_seqnum, 0);
        assert_eq!(au.index, 2);
        assert!(!au.is_interleaved);
        assert_eq!(au.cts_delta, Some(2i32));
        assert_eq!(au.dts_delta, Some(-2i32));
        assert!(!au.is_fragment);
        assert_eq!(au.data, payload[11..][..1]);

        assert!(iter.next().is_none());
    }

    #[test]
    fn header_one_au_unknown_size() {
        let mut parser = PayloadParser::new_for(ModeConfig {
            index_len: 2,
            ..Default::default()
        });

        // Header section size: 2 bytes.
        // Header:
        // * 2 bits: AU-index. (set to 0)
        let payload = &[0x00, 0x02, 0x00, 0x01, 0x02];
        let mut iter = parser.parse(payload, 0, TS0).unwrap();
        assert_eq!(iter.data.len(), 2);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.index, 0);
        assert!(!au.is_fragment); // <==
        assert_eq!(au.data, payload[3..][..2]);

        assert!(iter.next().is_none());
    }

    #[test]
    fn header_two_aus_unknown_size() {
        let mut parser = PayloadParser::new_for(ModeConfig {
            index_len: 1,
            index_delta_len: 2,
            ..Default::default()
        });

        // Header section size: 2 bytes.
        // First AU-header:
        // * 1 bit: AU-index. (set to 0)
        // Second AU-header:
        // * 2 bits: AU-index-delta. (set to 2)
        let payload = &[0x00, 0x03, 0x40, 0x01, 0x02, 0x03, 0x04];
        let mut iter = parser.parse(payload, 0, TS0).unwrap();
        assert_eq!(iter.data.len(), 4);

        let err = iter
            .next()
            .unwrap()
            .unwrap_err()
            .downcast::<Mpeg4GenericDepayError>()
            .unwrap();
        assert_eq!(err, MultipleAusUnknownSize);
    }

    #[test]
    fn header_two_aus_one_fragment() {
        let mut parser = PayloadParser::new_for(ModeConfig {
            size_len: 2,
            constant_duration: 2,
            ..Default::default()
        });

        // Header section size: 2 bytes.
        // Each AU-header:
        // * 2 bits: data size (AU1: 2 => b10.._...., AU2: 3 => b..11_....)
        let payload = &[0x00, 0x04, 0xb0, 0x01, 0x02, 0x03];
        let mut iter = parser.parse(payload, 0, TS0).unwrap();
        assert_eq!(iter.data.len(), 3);

        let au = iter.next().unwrap().unwrap();
        assert_eq!(au.index, 0);
        assert!(!au.is_interleaved);
        assert!(!au.is_fragment);
        assert_eq!(au.data, payload[3..][..2]);

        let err = iter
            .next()
            .unwrap()
            .unwrap_err()
            .downcast::<Mpeg4GenericDepayError>()
            .unwrap();
        assert_eq!(
            err,
            MultipleAusGreaterSizeThanAuData {
                au_size: 3,
                au_data_size: 1
            }
        );
    }

    #[test]
    fn header_section_too_large() {
        let mut parser = PayloadParser::new_for(ModeConfig {
            size_len: 4,
            ..Default::default()
        });

        // Header section size: 2 bytes.
        // Each Au-Header: 4 bits, so 3 of them here.
        let payload = &[0x00, 0x0c, 0x00];
        let err = parser
            .parse(payload, 0, TS0)
            .unwrap_err()
            .downcast::<Mpeg4GenericDepayError>()
            .unwrap();
        assert_eq!(
            err,
            AuHeaderSectionTooLarge {
                expected_end: 4,
                total: 3
            }
        );
    }

    #[test]
    fn auxiliary_section_too_large() {
        let mut parser = PayloadParser::new_for(ModeConfig {
            random_access_indication: true,
            auxiliary_data_size_len: 2,
            ..Default::default()
        });

        // Header section size: 2 bytes.
        // * 2x Au-Header: 3 bits each. 3 of them here + padding => 1 byte
        // Auxiliary section size: 2 bytes. (one byte available instead of 2 advertised)
        let payload = &[0x00, 0x06, 0x00, 0x00, 0x0c, 0x00];
        let err = parser
            .parse(payload, 0, TS0)
            .unwrap_err()
            .downcast::<Mpeg4GenericDepayError>()
            .unwrap();
        assert_eq!(
            err,
            AuAuxiliarySectionTooLarge {
                expected_end: 7,
                total: 6,
            }
        );
    }
}
