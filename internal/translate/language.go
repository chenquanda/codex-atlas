package translate

import (
	"strings"
	"unicode"
)

type Language string

const (
	LanguageChinese Language = "zh"
	LanguageEnglish Language = "en"
	LanguageZH      Language = LanguageChinese
	LanguageEN      Language = LanguageEnglish
	LanguageUnknown Language = "unknown"
)

func DetectLanguage(text string) Language {
	text = strings.TrimSpace(text)
	if text == "" {
		return LanguageUnknown
	}

	var chineseCount int
	var latinCount int
	for _, r := range text {
		switch {
		case isChineseRune(r):
			chineseCount++
		case unicode.IsLetter(r) && r <= unicode.MaxASCII:
			latinCount++
		}
	}

	meaningfulCount := chineseCount + latinCount
	if meaningfulCount < 8 {
		return LanguageUnknown
	}
	if chineseCount >= 4 {
		return LanguageZH
	}
	if chineseCount > 0 && float64(chineseCount)/float64(meaningfulCount) >= 0.2 {
		return LanguageZH
	}
	if latinCount >= 8 {
		return LanguageEN
	}
	return LanguageUnknown
}

func isChineseRune(r rune) bool {
	return (r >= 0x4E00 && r <= 0x9FFF) ||
		(r >= 0x3400 && r <= 0x4DBF) ||
		(r >= 0xF900 && r <= 0xFAFF)
}
