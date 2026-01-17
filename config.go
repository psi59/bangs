package main

import (
	"errors"
	"fmt"
	"net/url"
	"os"

	"gopkg.in/yaml.v3"
)

type BangConfig struct {
	Trigger     string `yaml:"trigger"`
	Name        string `yaml:"name"`
	URLTemplate string `yaml:"url_template"`
	HomeURL     string `yaml:"home_url"`
}

type Config struct {
	DefaultBang string       `yaml:"default_bang"`
	Bangs       []BangConfig `yaml:"bangs"`
}

func LoadConfigFromYAML(data []byte) (*Config, error) {
	var config Config
	if err := yaml.Unmarshal(data, &config); err != nil {
		return nil, fmt.Errorf("failed to parse YAML: %w", err)
	}
	if err := config.Validate(); err != nil {
		return nil, err
	}
	return &config, nil
}

func (c *Config) Validate() error {
	for i, bang := range c.Bangs {
		if bang.Trigger == "" {
			return fmt.Errorf("bang[%d]: trigger is required", i)
		}
		if bang.URLTemplate == "" {
			return errors.New("bang[" + bang.Trigger + "]: url_template is required")
		}
		if _, err := url.Parse(bang.URLTemplate); err != nil {
			return fmt.Errorf("bang[%s]: invalid url_template: %w", bang.Trigger, err)
		}
		parsed, _ := url.Parse(bang.URLTemplate)
		if parsed.Scheme == "" || parsed.Host == "" {
			return fmt.Errorf("bang[%s]: url_template must be a valid URL with scheme and host", bang.Trigger)
		}
	}
	return nil
}

func LoadConfigFromFile(path string) (*Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("failed to read config file: %w", err)
	}
	return LoadConfigFromYAML(data)
}

func (c *Config) ToRepository() *Repository {
	repo := NewRepository()
	for _, bc := range c.Bangs {
		bang := NewBang(bc.Trigger, bc.Name, bc.URLTemplate, bc.HomeURL)
		repo.Add(bang)
	}
	repo.SetDefault(c.DefaultBang)
	return repo
}
