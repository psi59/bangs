package main

import (
	"strings"
)

type Parser struct{}

type ParsedQuery struct {
	FirstWord  string
	SearchTerm string
}

func NewParser() *Parser {
	return &Parser{}
}

func (p *Parser) Parse(query string) ParsedQuery {
	query = strings.TrimSpace(query)
	if query == "" {
		return ParsedQuery{
			FirstWord:  "",
			SearchTerm: "",
		}
	}

	idx := strings.IndexAny(query, " \t")
	if idx == -1 {
		return ParsedQuery{
			FirstWord:  query,
			SearchTerm: "",
		}
	}

	return ParsedQuery{
		FirstWord:  query[:idx],
		SearchTerm: strings.TrimSpace(query[idx+1:]),
	}
}
