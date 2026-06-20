//! Access Unit Header and its parser & writer.

use bitstream_io::{BitRead, BitWrite, FromBitStreamWith, ToBitStreamWith};

use crate::mp4g::{AccessUnitIndex, ModeConfig};

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum AuHeaderError {
    #[error("Unexpected zero-sized AU {}", .0)]
    ZeroSizedAu(AccessUnitIndex),

    #[error("Undefined mandatory size for AU {}", .0)]
    UndefinedMandatorySize(AccessUnitIndex),

    #[error("Inconsistent delta index {index}. Previous index: {prev_index}")]
    InconsistentDeltaIndex {
        index: AccessUnitIndex,
        prev_index: AccessUnitIndex,
    },

    #[error("Unexpected CTS flag set for the first AU header {}", .0)]
    CtsFlagSetInFirstAuHeader(AccessUnitIndex),
}

#[derive(Debug)]
pub struct AuHeaderContext<'a> {
    pub(crate) config: &'a ModeConfig,
    pub(crate) prev_index: Option<AccessUnitIndex>,
}

#[derive(Debug, Default)]
pub struct AuHeader {
    pub(crate) size: Option<u32>,
    pub(crate) index: AccessUnitIndex,
    pub(crate) cts_delta: Option<i32>,
    pub(crate) dts_delta: Option<i32>,
    pub(crate) maybe_random_access: Option<bool>,
    pub(crate) is_interleaved: bool,
}

impl AuHeader {
    #[inline]
    pub(crate) fn new_with(index: impl Into<AccessUnitIndex>) -> Self {
        AuHeader {
            index: index.into(),
            ..Default::default()
        }
    }
}

impl<'a> FromBitStreamWith<'a> for AuHeader {
    type Context = AuHeaderContext<'a>;
    type Error = anyhow::Error;

    fn from_reader<R: BitRead + ?Sized>(
        r: &mut R,
        ctx: &AuHeaderContext,
    ) -> Result<Self, Self::Error> {
        use anyhow::Context;
        use AuHeaderError::*;

        let mut this = AuHeader::default();

        if ctx.config.size_len > 0 {
            let val = r
                .read_var::<u32>(ctx.config.size_len as u32)
                .context("AU-size")?;

            // Will ensure the size is non-zero after we get the index
            this.size = Some(val);
        }

        this.index = match ctx.prev_index {
            None => r
                .read_var::<u32>(ctx.config.index_len as u32)
                .context("AU-Index")?
                .into(),
            Some(prev_index) => {
                let delta = r
                    .read_var::<u32>(ctx.config.index_delta_len as u32)
                    .context("AU-Index-delta")?;
                if delta > 0 {
                    this.is_interleaved = true;
                }

                prev_index + 1u32 + delta
            }
        };

        if let Some(0) = this.size {
            Err(ZeroSizedAu(this.index))?;
        }

        if ctx.config.cts_delta_len > 0 && r.read_bit().context("CTS-flag")? {
            if ctx.prev_index.is_none() {
                // § 3.2.1.1:
                // > the CTS-flag field MUST have the value 0 in the first AU-header
                Err(CtsFlagSetInFirstAuHeader(this.index))?;
            }

            let delta = r
                .read_var::<i32>(ctx.config.cts_delta_len as u32)
                .context("CTS-delta")?;
            this.cts_delta = Some(delta);
        }

        if ctx.config.dts_delta_len > 0 && r.read_bit().context("DTS-flag")? {
            let delta = r
                .read_var::<i32>(ctx.config.dts_delta_len as u32)
                .context("DTS-delta")?;
            this.dts_delta = Some(delta);
        }

        if ctx.config.random_access_indication {
            this.maybe_random_access = Some(r.read_bit().context("RAP-flag")?);
        }

        // ignored by gstrtpmp4gdepay
        if ctx.config.stream_state_indication > 0 {
            r.skip(ctx.config.stream_state_indication as u32)
                .context("Stream-state")?;
        }

        Ok(this)
    }
}

impl<'a> ToBitStreamWith<'a> for AuHeader {
    type Context = AuHeaderContext<'a>;
    type Error = anyhow::Error;

    fn to_writer<W: BitWrite + ?Sized>(
        &self,
        w: &mut W,
        ctx: &AuHeaderContext,
    ) -> Result<(), Self::Error> {
        use anyhow::Context;
        use AuHeaderError::*;

        if ctx.config.size_len > 0 {
            let Some(size) = self.size else {
                return Err(UndefinedMandatorySize(self.index).into());
            };

            if size == 0 {
                Err(ZeroSizedAu(self.index))?;
            }

            w.write_var(ctx.config.size_len as u32, size)
                .context("AU-size")?;
        }

        match ctx.prev_index {
            None => w
                .write_var(ctx.config.index_len as u32, *self.index)
                .context("AU-Index")?,
            Some(prev_index) => {
                let index_delta = self
                    .index
                    .checked_sub(*prev_index)
                    .and_then(|delta| delta.checked_sub(1))
                    .ok_or(InconsistentDeltaIndex {
                        index: self.index,
                        prev_index,
                    })
                    .context("AU-Index-delta")?;
                w.write_var(ctx.config.index_delta_len as u32, index_delta)
                    .context("AU-Index-delta")?;
            }
        }

        if ctx.config.cts_delta_len > 0 {
            // § 3.2.1.1:
            // > the CTS-flag field MUST have the value 0 in the first AU-header
            // > the CTS-flag field SHOULD be 0 for any non-first fragment of an Access Unit
            if ctx.prev_index.is_none() {
                w.write_bit(false).context("CTS-flag")?;
            } else if let Some(cts_delta) = self.cts_delta {
                w.write_bit(true).context("CTS-flag")?;
                w.write_var(ctx.config.cts_delta_len as u32, cts_delta)
                    .context("CTS-delta")?;
            } else {
                w.write_bit(false).context("CTS-flag")?;
            }
        }

        if ctx.config.dts_delta_len > 0 {
            if let Some(dts_delta) = self.dts_delta {
                w.write_bit(true).context("DTS-flag")?;
                w.write_var(ctx.config.dts_delta_len as u32, dts_delta)
                    .context("DTS-delta")?;
            } else {
                w.write_bit(false).context("DTS-flag")?;
            }
        }

        if ctx.config.random_access_indication {
            w.write_bit(self.maybe_random_access.unwrap_or(false))
                .context("RAP-flag")?;
        }

        Ok(())
    }
}
