package main

import "testing"

func TestNewParser(t *testing.T) {
	parser := NewParser()

	if parser == nil {
		t.Error("expected parser to be non-nil")
	}
}

func TestParser_Parse_SingleWord(t *testing.T) {
	parser := NewParser()

	result := parser.Parse("hello")

	if result.FirstWord != "hello" {
		t.Errorf("expected FirstWord to be 'hello', got '%s'", result.FirstWord)
	}
	if result.SearchTerm != "" {
		t.Errorf("expected SearchTerm to be empty, got '%s'", result.SearchTerm)
	}
}

func TestParser_Parse_MultipleWords(t *testing.T) {
	parser := NewParser()

	result := parser.Parse("hello world")

	if result.FirstWord != "hello" {
		t.Errorf("expected FirstWord to be 'hello', got '%s'", result.FirstWord)
	}
	if result.SearchTerm != "world" {
		t.Errorf("expected SearchTerm to be 'world', got '%s'", result.SearchTerm)
	}
}

func TestParser_Parse_WithExclamation(t *testing.T) {
	parser := NewParser()

	result := parser.Parse("!g hello world")

	if result.FirstWord != "!g" {
		t.Errorf("expected FirstWord to be '!g', got '%s'", result.FirstWord)
	}
	if result.SearchTerm != "hello world" {
		t.Errorf("expected SearchTerm to be 'hello world', got '%s'", result.SearchTerm)
	}
}

func TestParser_Parse_TriggerOnly(t *testing.T) {
	parser := NewParser()

	result := parser.Parse("g")

	if result.FirstWord != "g" {
		t.Errorf("expected FirstWord to be 'g', got '%s'", result.FirstWord)
	}
	if result.SearchTerm != "" {
		t.Errorf("expected SearchTerm to be empty, got '%s'", result.SearchTerm)
	}
}

func TestParser_Parse_EmptyQuery(t *testing.T) {
	parser := NewParser()

	result := parser.Parse("")

	if result.FirstWord != "" {
		t.Errorf("expected FirstWord to be empty, got '%s'", result.FirstWord)
	}
	if result.SearchTerm != "" {
		t.Errorf("expected SearchTerm to be empty, got '%s'", result.SearchTerm)
	}
}

func TestParser_Parse_WhitespaceOnly(t *testing.T) {
	parser := NewParser()

	result := parser.Parse("   ")

	if result.FirstWord != "" {
		t.Errorf("expected FirstWord to be empty, got '%s'", result.FirstWord)
	}
	if result.SearchTerm != "" {
		t.Errorf("expected SearchTerm to be empty, got '%s'", result.SearchTerm)
	}
}

func TestParser_Parse_LeadingWhitespace(t *testing.T) {
	parser := NewParser()

	result := parser.Parse("  g hello")

	if result.FirstWord != "g" {
		t.Errorf("expected FirstWord to be 'g', got '%s'", result.FirstWord)
	}
	if result.SearchTerm != "hello" {
		t.Errorf("expected SearchTerm to be 'hello', got '%s'", result.SearchTerm)
	}
}

func TestParser_Parse_MultipleSpaces(t *testing.T) {
	parser := NewParser()

	result := parser.Parse("g   hello   world")

	if result.FirstWord != "g" {
		t.Errorf("expected FirstWord to be 'g', got '%s'", result.FirstWord)
	}
	if result.SearchTerm != "hello   world" {
		t.Errorf("expected SearchTerm to be 'hello   world', got '%s'", result.SearchTerm)
	}
}
