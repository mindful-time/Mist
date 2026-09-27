//! Kokoro voice metadata implementing the core voice-catalog port.

use crate::{
    domain::{LanguageId, LanguageProfile, MistPalette, VoiceProfile},
    ports::VoiceCatalog,
};

/// Catalog metadata owned by the Kokoro adapter.
#[derive(Clone, Copy, Debug, Default)]
pub struct KokoroVoiceCatalog;

const AMERICAN_ENGLISH: LanguageId = LanguageId::new("en-US");
const BRITISH_ENGLISH: LanguageId = LanguageId::new("en-GB");
const SPANISH: LanguageId = LanguageId::new("es");
const FRENCH: LanguageId = LanguageId::new("fr");
const HINDI: LanguageId = LanguageId::new("hi");
const ITALIAN: LanguageId = LanguageId::new("it");
const JAPANESE: LanguageId = LanguageId::new("ja");
const BRAZILIAN_PORTUGUESE: LanguageId = LanguageId::new("pt-BR");
const MANDARIN: LanguageId = LanguageId::new("zh-CN");

pub const KOKORO_LANGUAGES: &[LanguageProfile] = &[
    LanguageProfile {
        id: AMERICAN_ENGLISH,
        display_name: "English (US)",
    },
    LanguageProfile {
        id: BRITISH_ENGLISH,
        display_name: "English (UK)",
    },
    LanguageProfile {
        id: SPANISH,
        display_name: "Spanish",
    },
    LanguageProfile {
        id: FRENCH,
        display_name: "French",
    },
    LanguageProfile {
        id: HINDI,
        display_name: "Hindi",
    },
    LanguageProfile {
        id: ITALIAN,
        display_name: "Italian",
    },
    LanguageProfile {
        id: JAPANESE,
        display_name: "Japanese",
    },
    LanguageProfile {
        id: BRAZILIAN_PORTUGUESE,
        display_name: "Portuguese (BR)",
    },
    LanguageProfile {
        id: MANDARIN,
        display_name: "Mandarin",
    },
];

const ROSE: MistPalette = MistPalette {
    primary: [255, 130, 173],
    secondary: [247, 182, 226],
    glow: [255, 224, 239],
};
const VIOLET: MistPalette = MistPalette {
    primary: [163, 102, 255],
    secondary: [239, 111, 192],
    glow: [230, 210, 255],
};
const BLUE: MistPalette = MistPalette {
    primary: [84, 113, 255],
    secondary: [118, 212, 255],
    glow: [202, 226, 255],
};
const TEAL: MistPalette = MistPalette {
    primary: [82, 219, 203],
    secondary: [124, 157, 255],
    glow: [209, 255, 245],
};
const AMBER: MistPalette = MistPalette {
    primary: [255, 132, 72],
    secondary: [255, 198, 92],
    glow: [255, 230, 190],
};
const AQUA: MistPalette = MistPalette {
    primary: [35, 175, 205],
    secondary: [91, 231, 190],
    glow: [190, 251, 245],
};
const LILAC: MistPalette = MistPalette {
    primary: [193, 113, 255],
    secondary: [118, 154, 255],
    glow: [234, 218, 255],
};
const SLATE: MistPalette = MistPalette {
    primary: [101, 133, 164],
    secondary: [166, 199, 210],
    glow: [221, 236, 239],
};
const GOLD: MistPalette = MistPalette {
    primary: [244, 183, 64],
    secondary: [255, 122, 98],
    glow: [255, 238, 190],
};
const JADE: MistPalette = MistPalette {
    primary: [62, 193, 132],
    secondary: [111, 228, 201],
    glow: [204, 255, 229],
};
const CRIMSON: MistPalette = MistPalette {
    primary: [225, 76, 116],
    secondary: [255, 154, 107],
    glow: [255, 213, 220],
};
const INDIGO: MistPalette = MistPalette {
    primary: [90, 93, 214],
    secondary: [173, 126, 242],
    glow: [220, 218, 255],
};

macro_rules! language_id {
    (AmericanEnglish) => {
        AMERICAN_ENGLISH
    };
    (BritishEnglish) => {
        BRITISH_ENGLISH
    };
    (Spanish) => {
        SPANISH
    };
    (French) => {
        FRENCH
    };
    (Hindi) => {
        HINDI
    };
    (Italian) => {
        ITALIAN
    };
    (Japanese) => {
        JAPANESE
    };
    (BrazilianPortuguese) => {
        BRAZILIAN_PORTUGUESE
    };
    (Mandarin) => {
        MANDARIN
    };
}

macro_rules! voice {
    ($id:literal, $name:literal, $character:literal, $language:ident, $palette:ident) => {
        VoiceProfile {
            id: $id,
            display_name: $name,
            character: $character,
            language: language_id!($language),
            palette: $palette,
        }
    };
}

pub const KOKORO_VOICES: &[VoiceProfile] = &[
    voice!(
        "af_alloy",
        "Alloy",
        "Female · balanced",
        AmericanEnglish,
        SLATE
    ),
    voice!(
        "af_aoede",
        "Aoede",
        "Female · lyrical",
        AmericanEnglish,
        LILAC
    ),
    voice!(
        "af_bella",
        "Bella",
        "Female · expressive",
        AmericanEnglish,
        VIOLET
    ),
    voice!("af_heart", "Heart", "Female · warm", AmericanEnglish, ROSE),
    voice!(
        "af_jessica",
        "Jessica",
        "Female · clear",
        AmericanEnglish,
        TEAL
    ),
    voice!(
        "af_kore",
        "Kore",
        "Female · composed",
        AmericanEnglish,
        BLUE
    ),
    voice!(
        "af_nicole",
        "Nicole",
        "Female · intimate",
        AmericanEnglish,
        INDIGO
    ),
    voice!("af_nova", "Nova", "Female · bright", AmericanEnglish, AQUA),
    voice!("af_river", "River", "Female · calm", AmericanEnglish, JADE),
    voice!(
        "af_sarah",
        "Sarah",
        "Female · assured",
        AmericanEnglish,
        TEAL
    ),
    voice!("af_sky", "Sky", "Female · gentle", AmericanEnglish, BLUE),
    voice!("am_adam", "Adam", "Male · deep", AmericanEnglish, AMBER),
    voice!("am_echo", "Echo", "Male · resonant", AmericanEnglish, AQUA),
    voice!("am_eric", "Eric", "Male · direct", AmericanEnglish, SLATE),
    voice!(
        "am_fenrir",
        "Fenrir",
        "Male · textured",
        AmericanEnglish,
        CRIMSON
    ),
    voice!("am_liam", "Liam", "Male · relaxed", AmericanEnglish, JADE),
    voice!(
        "am_michael",
        "Michael",
        "Male · grounded",
        AmericanEnglish,
        AQUA
    ),
    voice!("am_onyx", "Onyx", "Male · dark", AmericanEnglish, INDIGO),
    voice!("am_puck", "Puck", "Male · playful", AmericanEnglish, GOLD),
    voice!("am_santa", "Santa", "Male · rounded", AmericanEnglish, ROSE),
    voice!(
        "bf_alice",
        "Alice",
        "Female · polished",
        BritishEnglish,
        ROSE
    ),
    voice!(
        "bf_emma",
        "Emma",
        "Female · luminous",
        BritishEnglish,
        LILAC
    ),
    voice!(
        "bf_isabella",
        "Isabella",
        "Female · elegant",
        BritishEnglish,
        VIOLET
    ),
    voice!("bf_lily", "Lily", "Female · light", BritishEnglish, TEAL),
    voice!(
        "bm_daniel",
        "Daniel",
        "Male · measured",
        BritishEnglish,
        SLATE
    ),
    voice!(
        "bm_fable",
        "Fable",
        "Male · narrative",
        BritishEnglish,
        INDIGO
    ),
    voice!(
        "bm_george",
        "George",
        "Male · classic",
        BritishEnglish,
        BLUE
    ),
    voice!(
        "bm_lewis",
        "Lewis",
        "Male · conversational",
        BritishEnglish,
        AQUA
    ),
    voice!("ef_dora", "Dora", "Female · vivid", Spanish, AMBER),
    voice!("em_alex", "Alex", "Male · open", Spanish, GOLD),
    voice!("em_santa", "Santa", "Male · warm", Spanish, CRIMSON),
    voice!("ff_siwis", "Siwis", "Female · refined", French, LILAC),
    voice!("hf_alpha", "Alpha", "Female · clear", Hindi, GOLD),
    voice!("hf_beta", "Beta", "Female · soft", Hindi, ROSE),
    voice!("hm_omega", "Omega", "Male · rich", Hindi, AMBER),
    voice!("hm_psi", "Psi", "Male · steady", Hindi, SLATE),
    voice!("if_sara", "Sara", "Female · bright", Italian, TEAL),
    voice!("im_nicola", "Nicola", "Male · lyrical", Italian, JADE),
    voice!("jf_alpha", "Alpha", "Female · clear", Japanese, ROSE),
    voice!(
        "jf_gongitsune",
        "Gongitsune",
        "Female · storyful",
        Japanese,
        GOLD
    ),
    voice!("jf_nezumi", "Nezumi", "Female · nimble", Japanese, AQUA),
    voice!(
        "jf_tebukuro",
        "Tebukuro",
        "Female · gentle",
        Japanese,
        LILAC
    ),
    voice!("jm_kumo", "Kumo", "Male · calm", Japanese, INDIGO),
    voice!(
        "pf_dora",
        "Dora",
        "Female · sunny",
        BrazilianPortuguese,
        GOLD
    ),
    voice!(
        "pm_alex",
        "Alex",
        "Male · natural",
        BrazilianPortuguese,
        JADE
    ),
    voice!(
        "pm_santa",
        "Santa",
        "Male · warm",
        BrazilianPortuguese,
        AMBER
    ),
    voice!("zf_xiaobei", "Xiaobei", "Female · delicate", Mandarin, ROSE),
    voice!("zf_xiaoni", "Xiaoni", "Female · clear", Mandarin, TEAL),
    voice!("zf_xiaoxiao", "Xiaoxiao", "Female · lively", Mandarin, AQUA),
    voice!("zf_xiaoyi", "Xiaoyi", "Female · calm", Mandarin, LILAC),
    voice!("zm_yunjian", "Yunjian", "Male · firm", Mandarin, SLATE),
    voice!("zm_yunxi", "Yunxi", "Male · fluid", Mandarin, BLUE),
    voice!("zm_yunxia", "Yunxia", "Male · warm", Mandarin, AMBER),
    voice!("zm_yunyang", "Yunyang", "Male · resonant", Mandarin, INDIGO),
];

impl VoiceCatalog for KokoroVoiceCatalog {
    fn languages(&self) -> &[LanguageProfile] {
        KOKORO_LANGUAGES
    }

    fn voices(&self) -> &[VoiceProfile] {
        KOKORO_VOICES
    }

    fn default_voice_id(&self) -> &str {
        "af_heart"
    }

    fn default_voice_for(&self, language: LanguageId) -> Option<&str> {
        match language {
            AMERICAN_ENGLISH => Some("af_heart"),
            BRITISH_ENGLISH => Some("bf_emma"),
            SPANISH => Some("ef_dora"),
            FRENCH => Some("ff_siwis"),
            HINDI => Some("hf_alpha"),
            ITALIAN => Some("if_sara"),
            JAPANESE => Some("jf_alpha"),
            BRAZILIAN_PORTUGUESE => Some("pf_dora"),
            MANDARIN => Some("zf_xiaoni"),
            _ => None,
        }
    }

    fn preview_text(&self, language: LanguageId) -> Option<&str> {
        match language {
            AMERICAN_ENGLISH | BRITISH_ENGLISH => {
                Some("Hello. This is how I'll bring your selected words to life.")
            }
            SPANISH => Some("Hola. Así daré vida a las palabras que selecciones."),
            FRENCH => Some("Bonjour. Voici la voix qui donnera vie à votre texte."),
            HINDI => Some("नमस्ते। मैं आपके चुने हुए शब्दों को इस आवाज़ में पढ़ूँगा।"),
            ITALIAN => Some("Ciao. Questa voce darà vita alle parole che selezioni."),
            JAPANESE => Some("こんにちは。選んだ文章をこの声で読み上げます。"),
            BRAZILIAN_PORTUGUESE => Some("Olá. Esta voz dará vida ao texto que você selecionar."),
            MANDARIN => Some("你好。我会用这个声音朗读你选择的文字。"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn catalog_has_unique_stable_ids_and_valid_defaults() {
        let catalog = KokoroVoiceCatalog;
        let ids = catalog
            .voices()
            .iter()
            .map(|voice| voice.id)
            .collect::<HashSet<_>>();
        assert_eq!(ids.len(), 54);
        assert_eq!(
            catalog.profile(catalog.default_voice_id()).unwrap().id,
            "af_heart"
        );
        for language in catalog.languages() {
            let default = catalog.default_voice_for(language.id).unwrap();
            assert_eq!(catalog.profile(default).unwrap().language, language.id);
            assert!(
                catalog
                    .preview_text(language.id)
                    .is_some_and(|text| !text.is_empty())
            );
        }
    }

    #[test]
    fn catalog_covers_every_kokoro_language_family() {
        let catalog = KokoroVoiceCatalog;
        let languages = catalog
            .voices()
            .iter()
            .map(|voice| voice.language)
            .collect::<HashSet<_>>();
        assert_eq!(languages.len(), catalog.languages().len());
    }
}
