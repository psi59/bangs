package main

import "testing"

func TestKoreanToQwerty_SingleConsonantJamo(t *testing.T) {
	got := KoreanToQwerty("ㅎ")
	expected := "g"
	if got != expected {
		t.Errorf("expected %q, got %q", expected, got)
	}
}

func TestKoreanToQwerty_SingleVowelJamo(t *testing.T) {
	got := KoreanToQwerty("ㅜ")
	expected := "n"
	if got != expected {
		t.Errorf("expected %q, got %q", expected, got)
	}
}

func TestKoreanToQwerty_SyllableWithoutJongsung(t *testing.T) {
	got := KoreanToQwerty("호")
	expected := "gh"
	if got != expected {
		t.Errorf("expected %q, got %q", expected, got)
	}
}

func TestKoreanToQwerty_SyllableWithJongsung(t *testing.T) {
	got := KoreanToQwerty("혿")
	expected := "ghe"
	if got != expected {
		t.Errorf("expected %q, got %q", expected, got)
	}
}

func TestKoreanToQwerty_NonKoreanPassthrough(t *testing.T) {
	got := KoreanToQwerty("gh")
	expected := "gh"
	if got != expected {
		t.Errorf("expected %q, got %q", expected, got)
	}
}

func TestKoreanToQwerty_EmptyString(t *testing.T) {
	got := KoreanToQwerty("")
	expected := ""
	if got != expected {
		t.Errorf("expected %q, got %q", expected, got)
	}
}

func TestKoreanToQwerty_MixedInput(t *testing.T) {
	got := KoreanToQwerty("ㅎa")
	expected := "ga"
	if got != expected {
		t.Errorf("expected %q, got %q", expected, got)
	}
}

func TestKoreanToQwerty_CompoundVowel(t *testing.T) {
	got := KoreanToQwerty("좌")
	expected := "whk"
	if got != expected {
		t.Errorf("expected %q, got %q", expected, got)
	}
}

func TestKoreanToQwerty_CompoundJongsung(t *testing.T) {
	got := KoreanToQwerty("닭")
	expected := "ekfr"
	if got != expected {
		t.Errorf("expected %q, got %q", expected, got)
	}
}

func TestKoreanToQwerty_DoubleConsonant(t *testing.T) {
	got := KoreanToQwerty("ㅃ")
	expected := "Q"
	if got != expected {
		t.Errorf("expected %q, got %q", expected, got)
	}
}

func TestKoreanToQwerty_MultiSyllable(t *testing.T) {
	got := KoreanToQwerty("기트")
	expected := "rlxm"
	if got != expected {
		t.Errorf("expected %q, got %q", expected, got)
	}
}
