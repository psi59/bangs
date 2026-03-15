package main

import "strings"

var compatJamoToKey = map[rune]string{
	// 자음 (U+3131~U+314E)
	'ㄱ': "r", 'ㄲ': "R", 'ㄴ': "s", 'ㄷ': "e", 'ㄸ': "E",
	'ㄹ': "f", 'ㅁ': "a", 'ㅂ': "q", 'ㅃ': "Q", 'ㅅ': "t",
	'ㅆ': "T", 'ㅇ': "d", 'ㅈ': "w", 'ㅉ': "W", 'ㅊ': "c",
	'ㅋ': "z", 'ㅌ': "x", 'ㅍ': "v", 'ㅎ': "g",
	// 모음 (U+314F~U+3163)
	'ㅏ': "k", 'ㅐ': "o", 'ㅑ': "i", 'ㅒ': "O", 'ㅓ': "j",
	'ㅔ': "p", 'ㅕ': "u", 'ㅖ': "P", 'ㅗ': "h", 'ㅘ': "hk",
	'ㅙ': "ho", 'ㅚ': "hl", 'ㅛ': "y", 'ㅜ': "n", 'ㅝ': "nj",
	'ㅞ': "np", 'ㅟ': "nl", 'ㅠ': "b", 'ㅡ': "m", 'ㅢ': "ml",
	'ㅣ': "l",
}

var chosungToKey = [19]string{
	"r", "R", "s", "e", "E", "f", "a", "q", "Q", "t",
	"T", "d", "w", "W", "c", "z", "x", "v", "g",
}

var jungsungToKey = [21]string{
	"k", "o", "i", "O", "j", "p", "u", "P", "h", "hk",
	"ho", "hl", "y", "n", "nj", "np", "nl", "b", "m", "ml", "l",
}

var jongsungToKey = [28]string{
	"", "r", "R", "rt", "s", "sw", "sg", "e", "f", "fr",
	"fa", "fq", "ft", "fx", "fv", "fg", "a", "q", "qt", "t",
	"T", "d", "w", "c", "z", "x", "v", "g",
}

func KoreanToQwerty(s string) string {
	var b strings.Builder
	for _, r := range s {
		if r >= 0xAC00 && r <= 0xD7A3 {
			code := r - 0xAC00
			cho := code / (21 * 28)
			jung := (code / 28) % 21
			jong := code % 28
			b.WriteString(chosungToKey[cho])
			b.WriteString(jungsungToKey[jung])
			b.WriteString(jongsungToKey[jong])
		} else if key, ok := compatJamoToKey[r]; ok {
			b.WriteString(key)
		} else {
			b.WriteRune(r)
		}
	}
	return b.String()
}
