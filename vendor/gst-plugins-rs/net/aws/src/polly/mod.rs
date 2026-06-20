// Copyright (C) 2024 Mathieu Duponchelle <mathieu@centricular.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v2.0.
// If a copy of the MPL was not distributed with this file, You can obtain one at
// <https://mozilla.org/MPL/2.0/>.
//
// SPDX-License-Identifier: MPL-2.0

use gst::glib;
use gst::prelude::*;

mod imp;

use std::sync::LazyLock;

static CAT: LazyLock<gst::DebugCategory> = LazyLock::new(|| {
    gst::DebugCategory::new(
        "awspolly",
        gst::DebugColorFlags::empty(),
        Some("AWS Polly element"),
    )
});

use aws_sdk_polly::types::{Engine, LanguageCode, VoiceId};

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstAwsPollyEngine")]
#[non_exhaustive]
pub enum AwsPollyEngine {
    #[enum_value(name = "Standard", nick = "standard")]
    Standard = 0,
    #[enum_value(name = "Neural", nick = "neural")]
    Neural = 1,
}

impl From<AwsPollyEngine> for Engine {
    fn from(val: AwsPollyEngine) -> Self {
        use AwsPollyEngine::*;
        match val {
            Standard => Engine::Standard,
            Neural => Engine::Neural,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstAwsPollyVoiceId")]
#[non_exhaustive]
pub enum AwsPollyVoiceId {
    #[enum_value(name = "Aditi", nick = "aditi")]
    Aditi,
    #[enum_value(name = "Amy", nick = "amy")]
    Amy,
    #[enum_value(name = "Aria", nick = "aria")]
    Aria,
    #[enum_value(name = "Arlet", nick = "arlet")]
    Arlet,
    #[enum_value(name = "Arthur", nick = "arthur")]
    Arthur,
    #[enum_value(name = "Astrid", nick = "astrid")]
    Astrid,
    #[enum_value(name = "Ayanda", nick = "ayanda")]
    Ayanda,
    #[enum_value(name = "Bianca", nick = "bianca")]
    Bianca,
    #[enum_value(name = "Brian", nick = "brian")]
    Brian,
    #[enum_value(name = "Camila", nick = "camila")]
    Camila,
    #[enum_value(name = "Carla", nick = "carla")]
    Carla,
    #[enum_value(name = "Carmen", nick = "carmen")]
    Carmen,
    #[enum_value(name = "Celine", nick = "celine")]
    Celine,
    #[enum_value(name = "Chantal", nick = "chantal")]
    Chantal,
    #[enum_value(name = "Conchita", nick = "conchita")]
    Conchita,
    #[enum_value(name = "Cristiano", nick = "cristiano")]
    Cristiano,
    #[enum_value(name = "Daniel", nick = "daniel")]
    Daniel,
    #[enum_value(name = "Dora", nick = "dora")]
    Dora,
    #[enum_value(name = "Emma", nick = "emma")]
    Emma,
    #[enum_value(name = "Enrique", nick = "enrique")]
    Enrique,
    #[enum_value(name = "Ewa", nick = "ewa")]
    Ewa,
    #[enum_value(name = "Filiz", nick = "filiz")]
    Filiz,
    #[enum_value(name = "Gabrielle", nick = "gabrielle")]
    Gabrielle,
    #[enum_value(name = "Geraint", nick = "geraint")]
    Geraint,
    #[enum_value(name = "Giorgio", nick = "giorgio")]
    Giorgio,
    #[enum_value(name = "Gwyneth", nick = "gwyneth")]
    Gwyneth,
    #[enum_value(name = "Hannah", nick = "hannah")]
    Hannah,
    #[enum_value(name = "Hans", nick = "hans")]
    Hans,
    #[enum_value(name = "Hiujin", nick = "hiujin")]
    Hiujin,
    #[enum_value(name = "Ines", nick = "ines")]
    Ines,
    #[enum_value(name = "Ivy", nick = "ivy")]
    Ivy,
    #[enum_value(name = "Jacek", nick = "jacek")]
    Jacek,
    #[enum_value(name = "Jan", nick = "jan")]
    Jan,
    #[enum_value(name = "Joanna", nick = "joanna")]
    Joanna,
    #[enum_value(name = "Joey", nick = "joey")]
    Joey,
    #[enum_value(name = "Justin", nick = "justin")]
    Justin,
    #[enum_value(name = "Kajal", nick = "kajal")]
    Kajal,
    #[enum_value(name = "Karl", nick = "karl")]
    Karl,
    #[enum_value(name = "Kendra", nick = "kendra")]
    Kendra,
    #[enum_value(name = "Kevin", nick = "kevin")]
    Kevin,
    #[enum_value(name = "Kimberly", nick = "kimberly")]
    Kimberly,
    #[enum_value(name = "Lea", nick = "lea")]
    Lea,
    #[enum_value(name = "Liam", nick = "liam")]
    Liam,
    #[enum_value(name = "Liv", nick = "liv")]
    Liv,
    #[enum_value(name = "Lotte", nick = "lotte")]
    Lotte,
    #[enum_value(name = "Lucia", nick = "lucia")]
    Lucia,
    #[enum_value(name = "Lupe", nick = "lupe")]
    Lupe,
    #[enum_value(name = "Mads", nick = "mads")]
    Mads,
    #[enum_value(name = "Maja", nick = "maja")]
    Maja,
    #[enum_value(name = "Marlene", nick = "marlene")]
    Marlene,
    #[enum_value(name = "Mathieu", nick = "mathieu")]
    Mathieu,
    #[enum_value(name = "Matthew", nick = "matthew")]
    Matthew,
    #[enum_value(name = "Maxim", nick = "maxim")]
    Maxim,
    #[enum_value(name = "Mia", nick = "mia")]
    Mia,
    #[enum_value(name = "Miguel", nick = "miguel")]
    Miguel,
    #[enum_value(name = "Mizuki", nick = "mizuki")]
    Mizuki,
    #[enum_value(name = "Naja", nick = "naja")]
    Naja,
    #[enum_value(name = "Nicole", nick = "nicole")]
    Nicole,
    #[enum_value(name = "Olivia", nick = "olivia")]
    Olivia,
    #[enum_value(name = "Pedro", nick = "pedro")]
    Pedro,
    #[enum_value(name = "Penelope", nick = "penelope")]
    Penelope,
    #[enum_value(name = "Raveena", nick = "raveena")]
    Raveena,
    #[enum_value(name = "Ricardo", nick = "ricardo")]
    Ricardo,
    #[enum_value(name = "Ruben", nick = "ruben")]
    Ruben,
    #[enum_value(name = "Russell", nick = "russell")]
    Russell,
    #[enum_value(name = "Salli", nick = "salli")]
    Salli,
    #[enum_value(name = "Seoyeon", nick = "seoyeon")]
    Seoyeon,
    #[enum_value(name = "Takumi", nick = "takumi")]
    Takumi,
    #[enum_value(name = "Tatyana", nick = "tatyana")]
    Tatyana,
    #[enum_value(name = "Vicki", nick = "vicki")]
    Vicki,
    #[enum_value(name = "Vitoria", nick = "vitoria")]
    Vitoria,
    #[enum_value(name = "Zeina", nick = "zeina")]
    Zeina,
    #[enum_value(name = "Zhiyu", nick = "zhiyu")]
    Zhiyu,
}

impl From<AwsPollyVoiceId> for VoiceId {
    fn from(val: AwsPollyVoiceId) -> Self {
        use AwsPollyVoiceId::*;
        match val {
            Aditi => VoiceId::Aditi,
            Amy => VoiceId::Amy,
            Aria => VoiceId::Aria,
            Arlet => VoiceId::Arlet,
            Arthur => VoiceId::Arthur,
            Astrid => VoiceId::Astrid,
            Ayanda => VoiceId::Ayanda,
            Bianca => VoiceId::Bianca,
            Brian => VoiceId::Brian,
            Camila => VoiceId::Camila,
            Carla => VoiceId::Carla,
            Carmen => VoiceId::Carmen,
            Celine => VoiceId::Celine,
            Chantal => VoiceId::Chantal,
            Conchita => VoiceId::Conchita,
            Cristiano => VoiceId::Cristiano,
            Daniel => VoiceId::Daniel,
            Dora => VoiceId::Dora,
            Emma => VoiceId::Emma,
            Enrique => VoiceId::Enrique,
            Ewa => VoiceId::Ewa,
            Filiz => VoiceId::Filiz,
            Gabrielle => VoiceId::Gabrielle,
            Geraint => VoiceId::Geraint,
            Giorgio => VoiceId::Giorgio,
            Gwyneth => VoiceId::Gwyneth,
            Hannah => VoiceId::Hannah,
            Hans => VoiceId::Hans,
            Hiujin => VoiceId::Hiujin,
            Ines => VoiceId::Ines,
            Ivy => VoiceId::Ivy,
            Jacek => VoiceId::Jacek,
            Jan => VoiceId::Jan,
            Joanna => VoiceId::Joanna,
            Joey => VoiceId::Joey,
            Justin => VoiceId::Justin,
            Kajal => VoiceId::Kajal,
            Karl => VoiceId::Karl,
            Kendra => VoiceId::Kendra,
            Kevin => VoiceId::Kevin,
            Kimberly => VoiceId::Kimberly,
            Lea => VoiceId::Lea,
            Liam => VoiceId::Liam,
            Liv => VoiceId::Liv,
            Lotte => VoiceId::Lotte,
            Lucia => VoiceId::Lucia,
            Lupe => VoiceId::Lupe,
            Mads => VoiceId::Mads,
            Maja => VoiceId::Maja,
            Marlene => VoiceId::Marlene,
            Mathieu => VoiceId::Mathieu,
            Matthew => VoiceId::Matthew,
            Maxim => VoiceId::Maxim,
            Mia => VoiceId::Mia,
            Miguel => VoiceId::Miguel,
            Mizuki => VoiceId::Mizuki,
            Naja => VoiceId::Naja,
            Nicole => VoiceId::Nicole,
            Olivia => VoiceId::Olivia,
            Pedro => VoiceId::Pedro,
            Penelope => VoiceId::Penelope,
            Raveena => VoiceId::Raveena,
            Ricardo => VoiceId::Ricardo,
            Ruben => VoiceId::Ruben,
            Russell => VoiceId::Russell,
            Salli => VoiceId::Salli,
            Seoyeon => VoiceId::Seoyeon,
            Takumi => VoiceId::Takumi,
            Tatyana => VoiceId::Tatyana,
            Vicki => VoiceId::Vicki,
            Vitoria => VoiceId::Vitoria,
            Zeina => VoiceId::Zeina,
            Zhiyu => VoiceId::Zhiyu,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstAwsPollyLanguageCode")]
#[non_exhaustive]
pub enum AwsPollyLanguageCode {
    #[enum_value(name = "None", nick = "none")]
    None,
    #[enum_value(name = "Arb", nick = "arb")]
    Arb,
    #[enum_value(name = "CaEs", nick = "ca-ES")]
    CaEs,
    #[enum_value(name = "CmnCn", nick = "cmn-CN")]
    CmnCn,
    #[enum_value(name = "CyGb", nick = "cy-GB")]
    CyGb,
    #[enum_value(name = "DaDk", nick = "da-DK")]
    DaDk,
    #[enum_value(name = "DeAt", nick = "de-AT")]
    DeAt,
    #[enum_value(name = "DeDe", nick = "de-DE")]
    DeDe,
    #[enum_value(name = "EnAu", nick = "en-AU")]
    EnAu,
    #[enum_value(name = "EnGb", nick = "en-GB")]
    EnGb,
    #[enum_value(name = "EnGbWls", nick = "en-GB-WLS")]
    EnGbWls,
    #[enum_value(name = "EnIn", nick = "en-IN")]
    EnIn,
    #[enum_value(name = "EnNz", nick = "en-NZ")]
    EnNz,
    #[enum_value(name = "EnUs", nick = "en-US")]
    EnUs,
    #[enum_value(name = "EnZa", nick = "en-ZA")]
    EnZa,
    #[enum_value(name = "EsEs", nick = "es-ES")]
    EsEs,
    #[enum_value(name = "EsMx", nick = "es-MX")]
    EsMx,
    #[enum_value(name = "EsUs", nick = "es-US")]
    EsUs,
    #[enum_value(name = "FrCa", nick = "fr-CA")]
    FrCa,
    #[enum_value(name = "FrFr", nick = "fr-FR")]
    FrFr,
    #[enum_value(name = "HiIn", nick = "hi-IN")]
    HiIn,
    #[enum_value(name = "IsIs", nick = "is-IS")]
    IsIs,
    #[enum_value(name = "ItIt", nick = "it-IT")]
    ItIt,
    #[enum_value(name = "JaJp", nick = "ja-JP")]
    JaJp,
    #[enum_value(name = "KoKr", nick = "ko-KR")]
    KoKr,
    #[enum_value(name = "NbNo", nick = "nb-NO")]
    NbNo,
    #[enum_value(name = "NlNl", nick = "nl-NL")]
    NlNl,
    #[enum_value(name = "PlPl", nick = "pl-PL")]
    PlPl,
    #[enum_value(name = "PtBr", nick = "pt-BR")]
    PtBr,
    #[enum_value(name = "PtPt", nick = "pt-PT")]
    PtPt,
    #[enum_value(name = "RoRo", nick = "ro-RO")]
    RoRo,
    #[enum_value(name = "RuRu", nick = "ru-RU")]
    RuRu,
    #[enum_value(name = "SvSe", nick = "sv-SE")]
    SvSe,
    #[enum_value(name = "TrTr", nick = "tr-TR")]
    TrTr,
    #[enum_value(name = "YueCn", nick = "yue-CN")]
    YueCn,
}

impl From<AwsPollyLanguageCode> for LanguageCode {
    fn from(val: AwsPollyLanguageCode) -> Self {
        use AwsPollyLanguageCode::*;
        match val {
            Arb => LanguageCode::Arb,
            CaEs => LanguageCode::CaEs,
            CmnCn => LanguageCode::CmnCn,
            CyGb => LanguageCode::CyGb,
            DaDk => LanguageCode::DaDk,
            DeAt => LanguageCode::DeAt,
            DeDe => LanguageCode::DeDe,
            EnAu => LanguageCode::EnAu,
            EnGb => LanguageCode::EnGb,
            EnGbWls => LanguageCode::EnGbWls,
            EnIn => LanguageCode::EnIn,
            EnNz => LanguageCode::EnNz,
            EnUs => LanguageCode::EnUs,
            EnZa => LanguageCode::EnZa,
            EsEs => LanguageCode::EsEs,
            EsMx => LanguageCode::EsMx,
            EsUs => LanguageCode::EsUs,
            FrCa => LanguageCode::FrCa,
            FrFr => LanguageCode::FrFr,
            HiIn => LanguageCode::HiIn,
            IsIs => LanguageCode::IsIs,
            ItIt => LanguageCode::ItIt,
            JaJp => LanguageCode::JaJp,
            KoKr => LanguageCode::KoKr,
            NbNo => LanguageCode::NbNo,
            NlNl => LanguageCode::NlNl,
            PlPl => LanguageCode::PlPl,
            PtBr => LanguageCode::PtBr,
            PtPt => LanguageCode::PtPt,
            RoRo => LanguageCode::RoRo,
            RuRu => LanguageCode::RuRu,
            SvSe => LanguageCode::SvSe,
            TrTr => LanguageCode::TrTr,
            YueCn => LanguageCode::YueCn,
            None => unreachable!(),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Clone, Copy, glib::Enum)]
#[repr(u32)]
#[enum_type(name = "GstAwsOverflow")]
#[non_exhaustive]
pub enum AwsOverflow {
    #[enum_value(name = "Clip", nick = "clip")]
    Clip = 0,
    #[enum_value(name = "Overlap", nick = "overlap")]
    Overlap = 1,
    #[enum_value(name = "Shift", nick = "shift")]
    Shift = 2,
    #[cfg(feature = "signalsmith_stretch")]
    #[enum_value(name = "Compress", nick = "compress")]
    Compress = 3,
}

glib::wrapper! {
    pub struct Polly(ObjectSubclass<imp::Polly>) @extends gst::Element, gst::Object;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    #[cfg(feature = "doc")]
    {
        AwsPollyEngine::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        AwsPollyVoiceId::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        AwsPollyLanguageCode::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
        AwsOverflow::static_type().mark_as_plugin_api(gst::PluginAPIFlags::empty());
    }
    gst::Element::register(
        Some(plugin),
        "awspolly",
        gst::Rank::NONE,
        Polly::static_type(),
    )
}
