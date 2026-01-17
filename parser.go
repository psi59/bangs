package main

import (
	"regexp"
	"strings"
)

type Parser struct {
	bangRegex *regexp.Regexp
}

type ParsedQuery struct {
	BangTrigger string
	SearchTerm  string
}

func NewParser() *Parser {
	return &Parser{
		bangRegex: regexp.MustCompile(`^!(\w+)`),
	}
}

func (p *Parser) Parse(query string) ParsedQuery {
	query = strings.TrimSpace(query)
	if query == "" {
		return ParsedQuery{
			BangTrigger: "",
			SearchTerm:  "",
		}
	}

	match := p.bangRegex.FindStringSubmatch(query)
	if match == nil {
		return ParsedQuery{
			BangTrigger: "",
			SearchTerm:  query,
		}
	}

	bangTrigger := match[1]
	searchTerm := strings.TrimSpace(query[len(match[0]):])

	return ParsedQuery{
		BangTrigger: bangTrigger,
		SearchTerm:  searchTerm,
	}
}
