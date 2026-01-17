package main

import "testing"

func TestNewRepository(t *testing.T) {
	repo := NewRepository()

	if repo == nil {
		t.Error("expected repository to be non-nil")
	}
}

func TestRepository_AddAndFind(t *testing.T) {
	repo := NewRepository()
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")

	repo.Add(bang)
	found := repo.FindByTrigger("g")

	if found == nil {
		t.Fatal("expected to find bang")
	}
	if found.Name != "Google" {
		t.Errorf("expected Name to be 'Google', got '%s'", found.Name)
	}
}

func TestRepository_FindNotFound(t *testing.T) {
	repo := NewRepository()

	found := repo.FindByTrigger("nonexistent")

	if found != nil {
		t.Error("expected nil for nonexistent trigger")
	}
}

func TestRepository_DefaultBang(t *testing.T) {
	repo := NewRepository()
	bang := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
	repo.Add(bang)
	repo.SetDefault("g")

	defaultBang := repo.GetDefault()

	if defaultBang == nil {
		t.Fatal("expected default bang to be set")
	}
	if defaultBang.Trigger != "g" {
		t.Errorf("expected default trigger to be 'g', got '%s'", defaultBang.Trigger)
	}
}

func TestRepository_DefaultBangNotSet(t *testing.T) {
	repo := NewRepository()

	defaultBang := repo.GetDefault()

	if defaultBang != nil {
		t.Error("expected default bang to be nil when not set")
	}
}

func TestRepository_Reload(t *testing.T) {
	repo := NewRepository()
	bang1 := NewBang("g", "Google", "https://www.google.com/search?q={{{s}}}", "")
	repo.Add(bang1)

	newBangs := []*Bang{
		NewBang("w", "Wikipedia", "https://en.wikipedia.org/wiki/Special:Search?search={{{s}}}", ""),
	}
	repo.Reload(newBangs, "w")

	if repo.FindByTrigger("g") != nil {
		t.Error("expected old bang 'g' to be removed after reload")
	}
	if repo.FindByTrigger("w") == nil {
		t.Error("expected new bang 'w' to exist after reload")
	}
	if repo.GetDefault() == nil || repo.GetDefault().Trigger != "w" {
		t.Error("expected default to be 'w' after reload")
	}
}
