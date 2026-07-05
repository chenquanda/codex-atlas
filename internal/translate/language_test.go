package translate

import "testing"

func TestDetectLanguage(t *testing.T) {
	tests := []struct {
		name string
		text string
		want Language
	}{
		{name: "中文", text: "这是一个用于整理技能说明的工具。", want: LanguageZH},
		{name: "英文", text: "Use this skill when you need to teach a concept.", want: LanguageEN},
		{name: "混合中文明显", text: "Use $teach 来创建一节简短课程，并保存学习记录。", want: LanguageZH},
		{name: "极短调用未知", text: "$teach", want: LanguageUnknown},
		{name: "空文本未知", text: "  \n\t", want: LanguageUnknown},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := DetectLanguage(tt.text); got != tt.want {
				t.Fatalf("DetectLanguage(%q) = %q, want %q", tt.text, got, tt.want)
			}
		})
	}
}
