use aws_sdk_transcribestreaming::types as sdk_types;

use serde::{Serialize, Serializer};
use serde_with::{serde_as, SerializeAs};

#[serde_as]
#[derive(serde_derive::Serialize)]
struct EntityDef {
    start_time: f64,
    end_time: f64,
    category: Option<String>,
    r#type: Option<String>,
    content: Option<String>,
    confidence: Option<f64>,
}

impl SerializeAs<sdk_types::Entity> for EntityDef {
    fn serialize_as<S>(value: &sdk_types::Entity, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        EntityDef {
            start_time: value.start_time,
            end_time: value.end_time,
            category: value.category.clone(),
            r#type: value.r#type.clone(),
            content: value.content.clone(),
            confidence: value.confidence,
        }
        .serialize(serializer)
    }
}

#[serde_as]
#[derive(serde_derive::Serialize)]
enum ItemTypeDef {
    Pronunciation,
    Punctuation,
    Unknown,
}

impl SerializeAs<sdk_types::ItemType> for ItemTypeDef {
    fn serialize_as<S>(value: &sdk_types::ItemType, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use sdk_types::ItemType::*;
        match value {
            Pronunciation => ItemTypeDef::Pronunciation,
            Punctuation => ItemTypeDef::Punctuation,
            _ => ItemTypeDef::Unknown,
        }
        .serialize(serializer)
    }
}

#[serde_as]
#[derive(serde_derive::Serialize)]
pub struct ItemDef {
    pub start_time: f64,
    pub end_time: f64,
    #[serde_as(as = "Option<ItemTypeDef>")]
    pub r#type: Option<sdk_types::ItemType>,
    pub content: Option<String>,
    pub vocabulary_filter_match: bool,
    pub speaker: Option<String>,
    pub confidence: Option<f64>,
    pub stable: Option<bool>,
}

impl SerializeAs<sdk_types::Item> for ItemDef {
    fn serialize_as<S>(value: &sdk_types::Item, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ItemDef {
            start_time: value.start_time,
            end_time: value.end_time,
            r#type: value.r#type.clone(),
            content: value.content.clone(),
            vocabulary_filter_match: value.vocabulary_filter_match,
            speaker: value.speaker.clone(),
            confidence: value.confidence,
            stable: value.stable,
        }
        .serialize(serializer)
    }
}

#[serde_as]
#[derive(serde_derive::Serialize)]
struct AlternativeDef {
    transcript: Option<String>,
    #[serde_as(as = "Option<Vec<ItemDef>>")]
    items: Option<Vec<sdk_types::Item>>,
    #[serde_as(as = "Option<Vec<EntityDef>>")]
    entities: Option<Vec<sdk_types::Entity>>,
}

impl SerializeAs<sdk_types::Alternative> for AlternativeDef {
    fn serialize_as<S>(value: &sdk_types::Alternative, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        AlternativeDef {
            transcript: value.transcript.clone(),
            items: value.items.clone(),
            entities: value.entities.clone(),
        }
        .serialize(serializer)
    }
}

#[serde_as]
#[derive(serde_derive::Serialize)]
struct LanguageWithScoreDef {
    #[serde_as(as = "Option<LanguageCodeDef>")]
    language_code: Option<sdk_types::LanguageCode>,
    score: f64,
}

impl SerializeAs<sdk_types::LanguageWithScore> for LanguageWithScoreDef {
    fn serialize_as<S>(
        value: &sdk_types::LanguageWithScore,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        LanguageWithScoreDef {
            language_code: value.language_code.clone(),
            score: value.score,
        }
        .serialize(serializer)
    }
}

#[serde_as]
#[derive(serde_derive::Serialize)]
enum LanguageCodeDef {
    AfZa,
    ArAe,
    ArSa,
    CaEs,
    CsCz,
    DaDk,
    DeCh,
    DeDe,
    ElGr,
    EnAb,
    EnAu,
    EnGb,
    EnIe,
    EnIn,
    EnNz,
    EnUs,
    EnWl,
    EnZa,
    EsEs,
    EsUs,
    EuEs,
    FaIr,
    FiFi,
    FrCa,
    FrFr,
    GlEs,
    HeIl,
    HiIn,
    HrHr,
    IdId,
    ItIt,
    JaJp,
    KoKr,
    LvLv,
    MsMy,
    NlNl,
    NoNo,
    PlPl,
    PtBr,
    PtPt,
    RoRo,
    RuRu,
    SkSk,
    SoSo,
    SrRs,
    SvSe,
    ThTh,
    TlPh,
    UkUa,
    ViVn,
    ZhCn,
    ZhHk,
    ZhTw,
    ZuZa,
    Unknown,
}

impl SerializeAs<sdk_types::LanguageCode> for LanguageCodeDef {
    fn serialize_as<S>(value: &sdk_types::LanguageCode, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use sdk_types::LanguageCode::*;
        match value {
            AfZa => LanguageCodeDef::AfZa,
            ArAe => LanguageCodeDef::ArAe,
            ArSa => LanguageCodeDef::ArSa,
            CaEs => LanguageCodeDef::CaEs,
            CsCz => LanguageCodeDef::CsCz,
            DaDk => LanguageCodeDef::DaDk,
            DeCh => LanguageCodeDef::DeCh,
            DeDe => LanguageCodeDef::DeDe,
            ElGr => LanguageCodeDef::ElGr,
            EnAb => LanguageCodeDef::EnAb,
            EnAu => LanguageCodeDef::EnAu,
            EnGb => LanguageCodeDef::EnGb,
            EnIe => LanguageCodeDef::EnIe,
            EnIn => LanguageCodeDef::EnIn,
            EnNz => LanguageCodeDef::EnNz,
            EnUs => LanguageCodeDef::EnUs,
            EnWl => LanguageCodeDef::EnWl,
            EnZa => LanguageCodeDef::EnZa,
            EsEs => LanguageCodeDef::EsEs,
            EsUs => LanguageCodeDef::EsUs,
            EuEs => LanguageCodeDef::EuEs,
            FaIr => LanguageCodeDef::FaIr,
            FiFi => LanguageCodeDef::FiFi,
            FrCa => LanguageCodeDef::FrCa,
            FrFr => LanguageCodeDef::FrFr,
            GlEs => LanguageCodeDef::GlEs,
            HeIl => LanguageCodeDef::HeIl,
            HiIn => LanguageCodeDef::HiIn,
            HrHr => LanguageCodeDef::HrHr,
            IdId => LanguageCodeDef::IdId,
            ItIt => LanguageCodeDef::ItIt,
            JaJp => LanguageCodeDef::JaJp,
            KoKr => LanguageCodeDef::KoKr,
            LvLv => LanguageCodeDef::LvLv,
            MsMy => LanguageCodeDef::MsMy,
            NlNl => LanguageCodeDef::NlNl,
            NoNo => LanguageCodeDef::NoNo,
            PlPl => LanguageCodeDef::PlPl,
            PtBr => LanguageCodeDef::PtBr,
            PtPt => LanguageCodeDef::PtPt,
            RoRo => LanguageCodeDef::RoRo,
            RuRu => LanguageCodeDef::RuRu,
            SkSk => LanguageCodeDef::SkSk,
            SoSo => LanguageCodeDef::SoSo,
            SrRs => LanguageCodeDef::SrRs,
            SvSe => LanguageCodeDef::SvSe,
            ThTh => LanguageCodeDef::ThTh,
            TlPh => LanguageCodeDef::TlPh,
            UkUa => LanguageCodeDef::UkUa,
            ViVn => LanguageCodeDef::ViVn,
            ZhCn => LanguageCodeDef::ZhCn,
            ZhHk => LanguageCodeDef::ZhHk,
            ZhTw => LanguageCodeDef::ZhTw,
            ZuZa => LanguageCodeDef::ZuZa,
            _ => LanguageCodeDef::Unknown,
        }
        .serialize(serializer)
    }
}

#[serde_as]
#[derive(serde_derive::Serialize)]
struct ResultDef {
    result_id: Option<String>,
    start_time: f64,
    end_time: f64,
    is_partial: bool,
    #[serde_as(as = "Option<Vec<AlternativeDef>>")]
    alternatives: Option<Vec<sdk_types::Alternative>>,
    channel_id: Option<String>,
    #[serde_as(as = "Option<LanguageCodeDef>")]
    language_code: Option<sdk_types::LanguageCode>,
    #[serde_as(as = "Option<Vec<LanguageWithScoreDef>>")]
    language_identification: Option<Vec<sdk_types::LanguageWithScore>>,
}

impl SerializeAs<sdk_types::Result> for ResultDef {
    fn serialize_as<S>(value: &sdk_types::Result, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let def = ResultDef {
            result_id: value.result_id.clone(),
            start_time: value.start_time,
            end_time: value.end_time,
            is_partial: value.is_partial,
            alternatives: value.alternatives.clone(),
            channel_id: value.channel_id.clone(),
            language_code: value.language_code.clone(),
            language_identification: value.language_identification.clone(),
        };
        def.serialize(serializer)
    }
}

#[serde_as]
#[derive(serde_derive::Serialize)]
pub struct TranscriptDef {
    #[serde_as(as = "Option<Vec<ResultDef>>")]
    pub results: Option<Vec<sdk_types::Result>>,
}
