package main

import "testing"

func TestNewParser(t *testing.T) {
	parser := NewParser()

	if parser == nil {
		t.Error("expected parser to be non-nil")
	}
}

func TestParser_Parse_NoBang(t *testing.T) {
	parser := NewParser()

	result := parser.Parse("hello world")

	if result.BangTrigger != "" {
		t.Errorf("expected BangTrigger to be empty, got '%s'", result.BangTrigger)
	}
	if result.SearchTerm != "hello world" {
		t.Errorf("expected SearchTerm to be 'hello world', got '%s'", result.SearchTerm)
	}
}

func TestParser_Parse_BangAtStart(t *testing.T) {
	parser := NewParser()

	result := parser.Parse("!g hello world")

	if result.BangTrigger != "g" {
		t.Errorf("expected BangTrigger to be 'g', got '%s'", result.BangTrigger)
	}
	if result.SearchTerm != "hello world" {
		t.Errorf("expected SearchTerm to be 'hello world', got '%s'", result.SearchTerm)
	}
}

func TestParser_Parse_BangAtEnd_NotRecognized(t *testing.T) {
	parser := NewParser()

	result := parser.Parse("hello world !g")

	if result.BangTrigger != "" {
		t.Errorf("expected BangTrigger to be empty (bang at end not supported), got '%s'", result.BangTrigger)
	}
	if result.SearchTerm != "hello world !g" {
		t.Errorf("expected SearchTerm to be 'hello world !g', got '%s'", result.SearchTerm)
	}
}

func TestParser_Parse_BangInMiddle_NotRecognized(t *testing.T) {
	parser := NewParser()

	result := parser.Parse("hello !g world")

	if result.BangTrigger != "" {
		t.Errorf("expected BangTrigger to be empty (bang in middle not supported), got '%s'", result.BangTrigger)
	}
	if result.SearchTerm != "hello !g world" {
		t.Errorf("expected SearchTerm to be 'hello !g world', got '%s'", result.SearchTerm)
	}
}

func TestParser_Parse_MultiCharBang(t *testing.T) {
	parser := NewParser()

	result := parser.Parse("!yt music video")

	if result.BangTrigger != "yt" {
		t.Errorf("expected BangTrigger to be 'yt', got '%s'", result.BangTrigger)
	}
	if result.SearchTerm != "music video" {
		t.Errorf("expected SearchTerm to be 'music video', got '%s'", result.SearchTerm)
	}
}

func TestParser_Parse_BangOnly(t *testing.T) {
	parser := NewParser()

	result := parser.Parse("!g")

	if result.BangTrigger != "g" {
		t.Errorf("expected BangTrigger to be 'g', got '%s'", result.BangTrigger)
	}
	if result.SearchTerm != "" {
		t.Errorf("expected SearchTerm to be empty, got '%s'", result.SearchTerm)
	}
}

func TestParser_Parse_MultipleBangs(t *testing.T) {
	parser := NewParser()

	result := parser.Parse("!g hello !w world")

	if result.BangTrigger != "g" {
		t.Errorf("expected BangTrigger to be 'g', got '%s'", result.BangTrigger)
	}
	if result.SearchTerm != "hello !w world" {
		t.Errorf("expected SearchTerm to be 'hello !w world', got '%s'", result.SearchTerm)
	}
}

func TestParser_Parse_EmptyQuery(t *testing.T) {
	parser := NewParser()

	result := parser.Parse("")

	if result.BangTrigger != "" {
		t.Errorf("expected BangTrigger to be empty, got '%s'", result.BangTrigger)
	}
	if result.SearchTerm != "" {
		t.Errorf("expected SearchTerm to be empty, got '%s'", result.SearchTerm)
	}
}

func TestParser_Parse_WhitespaceOnly(t *testing.T) {
	parser := NewParser()

	result := parser.Parse("   ")

	if result.BangTrigger != "" {
		t.Errorf("expected BangTrigger to be empty, got '%s'", result.BangTrigger)
	}
	if result.SearchTerm != "" {
		t.Errorf("expected SearchTerm to be empty, got '%s'", result.SearchTerm)
	}
}
