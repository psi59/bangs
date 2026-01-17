package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestLoadConfigFromYAML_SingleBang(t *testing.T) {
	yaml := `
default_bang: g
bangs:
  - trigger: g
    name: Google
    url_template: "https://www.google.com/search?q={{{s}}}"
`
	config, err := LoadConfigFromYAML([]byte(yaml))

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if config.DefaultBang != "g" {
		t.Errorf("expected DefaultBang to be 'g', got '%s'", config.DefaultBang)
	}
	if len(config.Bangs) != 1 {
		t.Errorf("expected 1 bang, got %d", len(config.Bangs))
	}
}

func TestLoadConfigFromYAML_MultipleBangs(t *testing.T) {
	yaml := `
default_bang: g
bangs:
  - trigger: g
    name: Google
    url_template: "https://www.google.com/search?q={{{s}}}"
  - trigger: w
    name: Wikipedia
    url_template: "https://en.wikipedia.org/wiki/Special:Search?search={{{s}}}"
  - trigger: yt
    name: YouTube
    url_template: "https://www.youtube.com/results?search_query={{{s}}}"
`
	config, err := LoadConfigFromYAML([]byte(yaml))

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(config.Bangs) != 3 {
		t.Errorf("expected 3 bangs, got %d", len(config.Bangs))
	}
}

func TestLoadConfigFromYAML_InvalidYAML(t *testing.T) {
	yaml := `invalid: yaml: : format`

	config, err := LoadConfigFromYAML([]byte(yaml))

	if err == nil {
		t.Error("expected error for invalid YAML")
	}
	if config != nil {
		t.Error("expected config to be nil for invalid YAML")
	}
}

func TestLoadConfigFromFile(t *testing.T) {
	tmpDir := t.TempDir()
	configPath := filepath.Join(tmpDir, "bangs.yaml")
	content := `
default_bang: g
bangs:
  - trigger: g
    name: Google
    url_template: "https://www.google.com/search?q={{{s}}}"
`
	if err := os.WriteFile(configPath, []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	config, err := LoadConfigFromFile(configPath)

	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if config == nil {
		t.Fatal("expected config to be non-nil")
	}
	if config.DefaultBang != "g" {
		t.Errorf("expected DefaultBang to be 'g', got '%s'", config.DefaultBang)
	}
}

func TestLoadConfigFromFile_NotFound(t *testing.T) {
	config, err := LoadConfigFromFile("/nonexistent/path/bangs.yaml")

	if err == nil {
		t.Error("expected error for nonexistent file")
	}
	if config != nil {
		t.Error("expected config to be nil for nonexistent file")
	}
}

func TestLoadConfigFromYAML_MissingTrigger(t *testing.T) {
	yaml := `
default_bang: g
bangs:
  - name: Google
    url_template: "https://www.google.com/search?q={{{s}}}"
`
	config, err := LoadConfigFromYAML([]byte(yaml))

	if err == nil {
		t.Error("expected error for missing trigger")
	}
	if config != nil {
		t.Error("expected config to be nil for missing trigger")
	}
}

func TestLoadConfigFromYAML_MissingURLTemplate(t *testing.T) {
	yaml := `
default_bang: g
bangs:
  - trigger: g
    name: Google
`
	config, err := LoadConfigFromYAML([]byte(yaml))

	if err == nil {
		t.Error("expected error for missing url_template")
	}
	if config != nil {
		t.Error("expected config to be nil for missing url_template")
	}
}

func TestLoadConfigFromYAML_InvalidURLTemplate(t *testing.T) {
	yaml := `
default_bang: g
bangs:
  - trigger: g
    name: Google
    url_template: "not-a-valid-url"
`
	config, err := LoadConfigFromYAML([]byte(yaml))

	if err == nil {
		t.Error("expected error for invalid url_template")
	}
	if config != nil {
		t.Error("expected config to be nil for invalid url_template")
	}
}
