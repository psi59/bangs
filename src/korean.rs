use crate::gocompat::decode_rune;

const CHOSUNG: [&str; 19] = [
    "r", "R", "s", "e", "E", "f", "a", "q", "Q", "t", "T", "d", "w", "W", "c", "z", "x", "v", "g",
];

const JUNGSUNG: [&str; 21] = [
    "k", "o", "i", "O", "j", "p", "u", "P", "h", "hk", "ho", "hl", "y", "n", "nj", "np", "nl", "b",
    "m", "ml", "l",
];

const JONGSUNG: [&str; 28] = [
    "", "r", "R", "rt", "s", "sw", "sg", "e", "f", "fr", "fa", "fq", "ft", "fx", "fv", "fg", "a",
    "q", "qt", "t", "T", "d", "w", "c", "z", "x", "v", "g",
];

fn compat_jamo_to_key(r: char) -> Option<&'static str> {
    Some(match r {
        // 자음 (U+3131~U+314E)
        'ㄱ' => "r",
        'ㄲ' => "R",
        'ㄴ' => "s",
        'ㄷ' => "e",
        'ㄸ' => "E",
        'ㄹ' => "f",
        'ㅁ' => "a",
        'ㅂ' => "q",
        'ㅃ' => "Q",
        'ㅅ' => "t",
        'ㅆ' => "T",
        'ㅇ' => "d",
        'ㅈ' => "w",
        'ㅉ' => "W",
        'ㅊ' => "c",
        'ㅋ' => "z",
        'ㅌ' => "x",
        'ㅍ' => "v",
        'ㅎ' => "g",
        // 모음 (U+314F~U+3163)
        'ㅏ' => "k",
        'ㅐ' => "o",
        'ㅑ' => "i",
        'ㅒ' => "O",
        'ㅓ' => "j",
        'ㅔ' => "p",
        'ㅕ' => "u",
        'ㅖ' => "P",
        'ㅗ' => "h",
        'ㅘ' => "hk",
        'ㅙ' => "ho",
        'ㅚ' => "hl",
        'ㅛ' => "y",
        'ㅜ' => "n",
        'ㅝ' => "nj",
        'ㅞ' => "np",
        'ㅟ' => "nl",
        'ㅠ' => "b",
        'ㅡ' => "m",
        'ㅢ' => "ml",
        'ㅣ' => "l",
        _ => return None,
    })
}

/// Maps Hangul typed on the 2-set (두벌식) layout back to the QWERTY keys
/// that produced it. Other characters pass through; invalid UTF-8 bytes
/// become U+FFFD, as Go's range-over-string does.
pub fn korean_to_qwerty(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let (r, w) = decode_rune(&s[i..]);
        let r = r.unwrap_or('\u{FFFD}');
        let code = r as u32;
        if (0xAC00..=0xD7A3).contains(&code) {
            let code = code - 0xAC00;
            out.extend_from_slice(CHOSUNG[(code / (21 * 28)) as usize].as_bytes());
            out.extend_from_slice(JUNGSUNG[((code / 28) % 21) as usize].as_bytes());
            out.extend_from_slice(JONGSUNG[(code % 28) as usize].as_bytes());
        } else if let Some(key) = compat_jamo_to_key(r) {
            out.extend_from_slice(key.as_bytes());
        } else {
            let mut buf = [0; 4];
            out.extend_from_slice(r.encode_utf8(&mut buf).as_bytes());
        }
        i += w;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::korean_to_qwerty;

    fn conv(s: &str) -> String {
        String::from_utf8(korean_to_qwerty(s.as_bytes())).unwrap()
    }

    #[test]
    fn converts_jamo() {
        assert_eq!(conv("ㅎ"), "g");
        assert_eq!(conv("ㅏ"), "k");
        assert_eq!(conv("ㄲ"), "R");
    }

    #[test]
    fn converts_syllables() {
        assert_eq!(conv("호"), "gh");
        assert_eq!(conv("한"), "gks");
        assert_eq!(conv("과"), "rhk");
        assert_eq!(conv("닭"), "ekfr");
        assert_eq!(conv("안녕"), "dkssud");
    }

    #[test]
    fn passes_through_other_text() {
        assert_eq!(conv(""), "");
        assert_eq!(conv("abc"), "abc");
        assert_eq!(conv("a호b"), "aghb");
        assert_eq!(korean_to_qwerty(b"\xffa"), "\u{FFFD}a".as_bytes());
    }
}
