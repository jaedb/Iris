//! MPEG-4 Generic mode.

use gst::caps::NoFeature;

use std::str::FromStr;

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum ModeError {
    #[error("sizelength & constantsize can't be both defined")]
    BothAuSizeLenAndConstantSize,

    #[error("no AU length parameters defined: need sizelength, constantsize, or constantduration")]
    MissingAuLengthParameters,

    #[error("indexlength > 0 but indexdeltalength not defined")]
    MandatoryIndexDeltaLength,
}

#[derive(Debug, Default)]
pub struct ModeConfig {
    pub(crate) size_len: u8,
    pub(crate) index_len: u8,
    pub(crate) index_delta_len: u8,
    pub(crate) cts_delta_len: u8,
    pub(crate) dts_delta_len: u8,
    pub(crate) random_access_indication: bool,
    pub(crate) stream_state_indication: u8,
    pub(crate) auxiliary_data_size_len: u8,
    pub(crate) constant_size: u32,
    pub(crate) constant_duration: u32,
    pub(crate) max_displacement: u32,
}

impl ModeConfig {
    #[inline]
    pub fn has_header_section(&self) -> bool {
        self.size_len > 0
            || self.index_len > 0
            || self.index_delta_len > 0
            || self.cts_delta_len > 0
            || self.dts_delta_len > 0
            || self.random_access_indication
            || self.stream_state_indication > 0
    }

    #[inline]
    pub fn has_auxiliary_section(&self) -> bool {
        self.auxiliary_data_size_len > 0
    }

    #[inline]
    pub fn constant_duration(&self) -> Option<u32> {
        if self.constant_duration == 0 {
            return None;
        }

        Some(self.constant_duration)
    }

    #[inline]
    pub fn max_displacement(&self) -> Option<u32> {
        if self.max_displacement == 0 {
            return None;
        }

        Some(self.max_displacement)
    }

    /// Returns the max length in bits of the AU headers
    pub fn max_header_bit_len(&self) -> usize {
        self.size_len as usize
            + std::cmp::max(self.index_len, self.index_delta_len) as usize
            + self.cts_delta_len as usize
            + self.dts_delta_len as usize
            + if self.random_access_indication { 1 } else { 0 }
            + self.stream_state_indication as usize
    }

    pub fn from_caps(s: &gst::StructureRef) -> anyhow::Result<Self> {
        use ModeError::*;

        // These values are optional and have a default value of 0 (no header)

        let size_len = Self::parse_int::<u8>(s, "sizelength")?;
        let constant_size = Self::parse_int::<u32>(s, "constantsize")?;
        let constant_duration = Self::parse_int::<u32>(s, "constantduration")?;

        if size_len != 0 && constant_size != 0 {
            Err(BothAuSizeLenAndConstantSize)?;
        }

        if size_len == 0 && constant_size == 0 && constant_duration == 0 {
            Err(MissingAuLengthParameters)?;
        }

        // § 3.2.1
        // > If the AU-Index field is present in the first AU-header in the AU
        // > Header Section, then the AU-Index-delta field MUST be present in
        // > any subsequent (non-first) AU-header.

        let index_len = Self::parse_int::<u8>(s, "indexlength")?;
        let index_delta_len = Self::parse_int::<u8>(s, "indexdeltalength")?;

        if index_len > 0 && index_delta_len == 0 {
            Err(MandatoryIndexDeltaLength)?;
        }

        // TODO check mode & mode_config conformity

        Ok(ModeConfig {
            size_len,
            index_len,
            index_delta_len,
            cts_delta_len: Self::parse_int::<u8>(s, "ctsdeltalength")?,
            dts_delta_len: Self::parse_int::<u8>(s, "dtsdeltalength")?,
            random_access_indication: Self::parse_int::<u8>(s, "randomaccessindication")? > 0,
            stream_state_indication: Self::parse_int::<u8>(s, "streamstateindication")?,
            auxiliary_data_size_len: Self::parse_int::<u8>(s, "auxiliarydatasizelength")?,
            constant_size,
            constant_duration,
            max_displacement: Self::parse_int::<u32>(s, "maxdisplacement")?,
        })
    }

    /// Tries to read the `field` from the provided structure as an integer of type `T`.
    ///
    /// Returns:
    ///
    /// * `Ok(val)` if the field is present and its value could be parsed.
    /// * `Ok(0)` if the field is not present.
    /// * `Err(_)` otherwise.
    fn parse_int<'a, T>(s: &'a gst::StructureRef, field: &'static str) -> anyhow::Result<T>
    where
        T: TryFrom<i32> + FromStr + gst::glib::value::FromValue<'a>,
        <T as TryFrom<i32>>::Error: std::error::Error + Send + Sync + 'static,
        <T as FromStr>::Err: std::error::Error + Send + Sync + 'static,
    {
        use anyhow::Context;
        use gst::structure::GetError::*;

        match s.get::<T>(field) {
            Ok(val) => Ok(val),
            Err(FieldNotFound { .. }) => Ok(T::try_from(0i32).unwrap()),
            Err(ValueGetError { .. }) => match s.get::<i32>(field) {
                Ok(val) => Ok(T::try_from(val).context(field)?),
                Err(_) => Ok(s
                    .get::<&str>(field)
                    .context(field)?
                    .parse::<T>()
                    .context(field)?),
            },
        }
    }

    pub fn add_to_caps(
        &self,
        builder: gst::caps::Builder<NoFeature>,
    ) -> Result<gst::caps::Builder<NoFeature>, ModeError> {
        use ModeError::*;

        if self.size_len != 0 && self.constant_size != 0 {
            Err(BothAuSizeLenAndConstantSize)?;
        }

        if self.size_len == 0 && self.constant_size == 0 && self.constant_duration == 0 {
            Err(MissingAuLengthParameters)?;
        }

        if self.index_len > 0 && self.index_delta_len == 0 {
            Err(MandatoryIndexDeltaLength)?;
        }

        if self.stream_state_indication > 0 {
            panic!("AU Header Stream State not supported");
        }

        Ok(builder
            .field("sizelength", self.size_len as i32)
            .field("indexlength", self.index_len as i32)
            .field("indexdeltalength", self.index_delta_len as i32)
            .field("ctsdeltalength", self.cts_delta_len as i32)
            .field("dtsdeltalength", self.dts_delta_len as i32)
            .field(
                "randomaccessindication",
                if self.random_access_indication {
                    1u8
                } else {
                    0u8
                },
            )
            .field("streamstateindication", self.stream_state_indication as i32)
            .field(
                "auxiliarydatasizelength",
                self.auxiliary_data_size_len as i32,
            )
            .field("constantsize", self.constant_size as i32)
            .field("constantduration", self.constant_duration as i32)
            .field("maxdisplacement", self.max_displacement as i32))
    }
}
