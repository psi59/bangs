package main

import (
	"net/url"
	"strings"
)

type Bang struct {
	Trigger     string
	Name        string
	URLTemplate string
	HomeURL     string
}

func NewBang(trigger, name, urlTemplate, homeURL string) *Bang {
	return &Bang{
		Trigger:     trigger,
		Name:        name,
		URLTemplate: urlTemplate,
		HomeURL:     homeURL,
	}
}

func (b *Bang) BuildURL(query string) string {
	if query == "" {
		return b.GetHomeURL()
	}
	encoded := url.QueryEscape(query)
	return strings.Replace(b.URLTemplate, "{{{s}}}", encoded, 1)
}

func (b *Bang) GetHomeURL() string {
	if b.HomeURL != "" {
		return b.HomeURL
	}
	parsed, err := url.Parse(b.URLTemplate)
	if err != nil {
		return ""
	}
	return parsed.Scheme + "://" + parsed.Host
}
