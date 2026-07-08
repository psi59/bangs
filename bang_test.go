package main

import "testing"

func TestNewBang(t *testing.T) {
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")

	if bang.Trigger != "g" {
		t.Errorf("expected Trigger to be 'g', got '%s'", bang.Trigger)
	}
	if bang.Name != "Google" {
		t.Errorf("expected Name to be 'Google', got '%s'", bang.Name)
	}
	if bang.URLTemplate != "https://www.google.com/search?q={{{s}}}" {
		t.Errorf("expected URLTemplate to be 'https://www.google.com/search?q={{{s}}}', got '%s'", bang.URLTemplate)
	}
	if bang.HomeURL != "" {
		t.Errorf("expected HomeURL to be empty, got '%s'", bang.HomeURL)
	}
}

func TestBang_BuildURL_SimpleQuery(t *testing.T) {
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")

	url := bang.BuildURL("hello")

	expected := "https://www.google.com/search?q=hello"
	if url != expected {
		t.Errorf("expected '%s', got '%s'", expected, url)
	}
}

func TestBang_BuildURL_QueryWithSpaces(t *testing.T) {
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")

	url := bang.BuildURL("hello world")

	expected := "https://www.google.com/search?q=hello+world"
	if url != expected {
		t.Errorf("expected '%s', got '%s'", expected, url)
	}
}

func TestBang_BuildURL_QueryWithSpecialChars(t *testing.T) {
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")

	url := bang.BuildURL("hello & goodbye")

	expected := "https://www.google.com/search?q=hello+%26+goodbye"
	if url != expected {
		t.Errorf("expected '%s', got '%s'", expected, url)
	}
}

func TestBang_BuildURL_EmptyQuery_ReturnsHomeURL(t *testing.T) {
	bang := NewBang("yt", "YouTube", "https://www.youtube.com/results?search_query={{{s}}}", "https://www.youtube.com")

	url := bang.BuildURL("")

	expected := "https://www.youtube.com"
	if url != expected {
		t.Errorf("expected '%s', got '%s'", expected, url)
	}
}

func TestBang_BuildSuggestURL_WithTemplate(t *testing.T) {
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
	bang.SuggestURLTemplate = "https://suggestqueries.google.com/complete/search?client=firefox&q={{{s}}}"

	url := bang.BuildSuggestURL("hello world")

	expected := "https://suggestqueries.google.com/complete/search?client=firefox&q=hello+world"
	if url != expected {
		t.Errorf("expected '%s', got '%s'", expected, url)
	}
}

func TestBang_BuildSuggestURL_NoTemplate_ReturnsEmpty(t *testing.T) {
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")

	url := bang.BuildSuggestURL("hello")

	if url != "" {
		t.Errorf("expected empty string, got '%s'", url)
	}
}

func TestBang_GetHomeURL_WhenSet(t *testing.T) {
	bang := NewBang("yt", "YouTube", "https://www.youtube.com/results?search_query={{{s}}}", "https://www.youtube.com")

	homeURL := bang.GetHomeURL()

	expected := "https://www.youtube.com"
	if homeURL != expected {
		t.Errorf("expected '%s', got '%s'", expected, homeURL)
	}
}

func TestBang_GetHomeURL_WhenNotSet_ExtractsFromURLTemplate(t *testing.T) {
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")

	homeURL := bang.GetHomeURL()

	expected := "https://www.google.com"
	if homeURL != expected {
		t.Errorf("expected '%s', got '%s'", expected, homeURL)
	}
}
