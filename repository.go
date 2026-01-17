package main

import "sync"

type Repository struct {
	mu          sync.RWMutex
	bangs       map[string]*Bang
	defaultBang *Bang
}

func NewRepository() *Repository {
	return &Repository{
		bangs: make(map[string]*Bang),
	}
}

func (r *Repository) Add(bang *Bang) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.bangs[bang.Trigger] = bang
}

func (r *Repository) FindByTrigger(trigger string) *Bang {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return r.bangs[trigger]
}

func (r *Repository) SetDefault(trigger string) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.defaultBang = r.bangs[trigger]
}

func (r *Repository) GetDefault() *Bang {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return r.defaultBang
}

func (r *Repository) Reload(bangs []*Bang, defaultTrigger string) {
	r.mu.Lock()
	defer r.mu.Unlock()

	r.bangs = make(map[string]*Bang)
	for _, bang := range bangs {
		r.bangs[bang.Trigger] = bang
	}
	r.defaultBang = r.bangs[defaultTrigger]
}
