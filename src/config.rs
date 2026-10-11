//! bangs.yaml loading. Decoding follows gopkg.in/yaml.v3 as used by the Go
//! version: any scalar decodes into a string field as its literal text,
//! nulls decode to "", unknown keys are ignored, duplicate keys and type
//! mismatches are errors, and only the first document is read.

use std::collections::HashMap;
use std::io::Read;

use saphyr_parser::{Event, Parser, ScalarStyle, Tag};

use crate::bang::{Bang, Repository};
use crate::gocompat::parse_url;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct BangConfig {
    pub trigger: String,
    pub name: String,
    pub url_template: String,
    pub home_url: String,
    pub suggest_url_template: String,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Config {
    pub default_bang: String,
    pub bangs: Vec<BangConfig>,
}

impl Config {
    pub fn from_yaml(data: &[u8]) -> Result<Self, String> {
        let config = decode(data).map_err(|e| format!("failed to parse YAML: yaml: {e}"))?;
        config.validate()?;
        Ok(config)
    }

    pub fn from_file(path: &str) -> Result<Self, String> {
        let data = read_file(path).map_err(|e| format!("failed to read config file: {e}"))?;
        Self::from_yaml(&data)
    }

    pub fn validate(&self) -> Result<(), String> {
        for (i, bang) in self.bangs.iter().enumerate() {
            if bang.trigger.is_empty() {
                return Err(format!("bang[{i}]: trigger is required"));
            }
            let t = &bang.trigger;
            if bang.url_template.is_empty() {
                return Err(format!("bang[{t}]: url_template is required"));
            }
            let parsed = parse_url(&bang.url_template)
                .map_err(|e| format!("bang[{t}]: invalid url_template: {e}"))?;
            if parsed.scheme.is_empty() || parsed.host.is_empty() {
                return Err(format!(
                    "bang[{t}]: url_template must be a valid URL with scheme and host"
                ));
            }
            if !bang.suggest_url_template.is_empty() {
                let parsed = parse_url(&bang.suggest_url_template)
                    .map_err(|e| format!("bang[{t}]: invalid suggest_url_template: {e}"))?;
                if parsed.scheme.is_empty() || parsed.host.is_empty() {
                    return Err(format!(
                        "bang[{t}]: suggest_url_template must be a valid URL with scheme and host"
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn to_repository(&self) -> Repository {
        let mut repo = Repository::new();
        for bc in &self.bangs {
            let mut bang = Bang::new(&bc.trigger, &bc.name, &bc.url_template, &bc.home_url);
            bang.suggest_url_template = bc.suggest_url_template.clone();
            repo.add(bang);
        }
        repo.set_default(&self.default_bang);
        repo
    }
}

/// os.ReadFile with Go-style error text ("open <path>: <reason>").
fn read_file(path: &str) -> Result<Vec<u8>, String> {
    let describe = |op: &str, e: std::io::Error| {
        let reason = match e.raw_os_error() {
            Some(code) => {
                let msg = std::io::Error::from_raw_os_error(code).to_string();
                let msg = msg.split(" (os error").next().unwrap_or(&msg).to_owned();
                let mut chars = msg.chars();
                chars
                    .next()
                    .map(|c| c.to_lowercase().chain(chars).collect())
                    .unwrap_or(msg)
            }
            None => e.to_string(),
        };
        format!("{op} {path}: {reason}")
    };
    let mut file = std::fs::File::open(path).map_err(|e| describe("open", e))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|e| describe("read", e))?;
    Ok(data)
}

// ---------------------------------------------------------------------------
// YAML node tree

#[derive(Debug, Clone)]
enum Node {
    Scalar {
        value: String,
        style: ScalarStyle,
        tag: Option<Tag>,
        line: usize,
    },
    Seq(Vec<Node>, usize),
    Map(Vec<(Node, Node)>, usize),
}

impl Node {
    fn line(&self) -> usize {
        match self {
            Node::Scalar { line, .. } | Node::Seq(_, line) | Node::Map(_, line) => *line,
        }
    }

    fn is_null(&self) -> bool {
        match self {
            Node::Scalar {
                value, style, tag, ..
            } => match tag {
                Some(t) => t.is_yaml_core_schema() && t.suffix == "null",
                None => {
                    *style == ScalarStyle::Plain
                        && matches!(value.as_str(), "" | "~" | "null" | "Null" | "NULL")
                }
            },
            _ => false,
        }
    }

    fn is_merge_key(&self) -> bool {
        match self {
            Node::Scalar {
                value, style, tag, ..
            } => match tag {
                Some(t) => t.is_yaml_core_schema() && t.suffix == "merge",
                None => *style == ScalarStyle::Plain && value == "<<",
            },
            _ => false,
        }
    }

    fn describe(&self) -> String {
        match self {
            Node::Scalar { value, .. } => format!("!!str `{value}`"),
            Node::Seq(..) => "!!seq".into(),
            Node::Map(..) => "!!map".into(),
        }
    }
}

enum Building {
    Seq(Vec<Node>, usize, usize),
    Map(Vec<(Node, Node)>, Option<Node>, usize, usize),
}

/// Parses the first YAML document into a node tree (None when empty).
fn first_document(data: &[u8]) -> Result<Option<Node>, String> {
    let text = std::str::from_utf8(data).map_err(|_| "invalid leading UTF-8 octet".to_string())?;
    let mut anchors: HashMap<usize, Node> = HashMap::new();
    let mut stack: Vec<Building> = Vec::new();
    let mut root: Option<Node> = None;
    for item in Parser::new_from_str(text) {
        let (event, span) = item.map_err(|e| e.to_string())?;
        let line = span.start.line();
        let (node, anchor) = match event {
            Event::Scalar(value, style, anchor, tag) => (
                Node::Scalar {
                    value: value.into_owned(),
                    style,
                    tag: tag.map(|t| t.into_owned()),
                    line,
                },
                anchor,
            ),
            Event::Alias(id) => {
                let node = anchors
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| format!("line {line}: unknown anchor"))?;
                (node, 0)
            }
            Event::SequenceStart(anchor, _) => {
                stack.push(Building::Seq(Vec::new(), anchor, line));
                continue;
            }
            Event::MappingStart(anchor, _) => {
                stack.push(Building::Map(Vec::new(), None, anchor, line));
                continue;
            }
            Event::SequenceEnd => match stack.pop() {
                Some(Building::Seq(items, anchor, line)) => (Node::Seq(items, line), anchor),
                _ => return Err("unbalanced sequence".into()),
            },
            Event::MappingEnd => match stack.pop() {
                Some(Building::Map(pairs, _, anchor, line)) => (Node::Map(pairs, line), anchor),
                _ => return Err("unbalanced mapping".into()),
            },
            Event::DocumentEnd | Event::StreamEnd => break,
            _ => continue,
        };
        if anchor != 0 {
            anchors.insert(anchor, node.clone());
        }
        match stack.last_mut() {
            None => {
                root = Some(node);
            }
            Some(Building::Seq(items, ..)) => items.push(node),
            Some(Building::Map(pairs, pending, ..)) => match pending.take() {
                None => *pending = Some(node),
                Some(key) => pairs.push((key, node)),
            },
        }
    }
    Ok(root)
}

// ---------------------------------------------------------------------------
// Decoding into Config

fn decode(data: &[u8]) -> Result<Config, String> {
    let mut config = Config::default();
    let Some(root) = first_document(data)? else {
        return Ok(config);
    };
    if root.is_null() {
        return Ok(config);
    }
    let mut errors = Vec::new();
    for (key, value) in struct_fields(&root, "main.Config", &mut errors) {
        match key.as_str() {
            "default_bang" => config.default_bang = decode_string(value, &mut errors),
            "bangs" => config.bangs = decode_bangs(value, &mut errors),
            _ => {}
        }
    }
    if errors.is_empty() {
        Ok(config)
    } else {
        Err(format!("unmarshal errors:\n  {}", errors.join("\n  ")))
    }
}

/// Flattens a mapping decoded into a Go struct: explicit keys first, then
/// merged (`<<`) mappings for keys not already present.
fn struct_fields<'a>(
    node: &'a Node,
    type_name: &str,
    errors: &mut Vec<String>,
) -> Vec<(String, &'a Node)> {
    let Node::Map(pairs, _) = node else {
        errors.push(format!(
            "line {}: cannot unmarshal {} into {type_name}",
            node.line(),
            node.describe()
        ));
        return Vec::new();
    };
    check_unique_keys(pairs, errors);
    let mut fields: Vec<(String, &Node)> = Vec::new();
    let mut merges: Vec<&Node> = Vec::new();
    for (k, v) in pairs {
        if k.is_merge_key() {
            match v {
                Node::Seq(items, _) => merges.extend(items.iter()),
                other => merges.push(other),
            }
            continue;
        }
        match k {
            Node::Scalar { .. } => fields.push((
                if k.is_null() {
                    String::new()
                } else {
                    scalar_text(k)
                },
                v,
            )),
            other => errors.push(format!(
                "line {}: cannot unmarshal {} into string",
                other.line(),
                other.describe()
            )),
        }
    }
    for m in merges {
        for (k, v) in struct_fields(m, type_name, errors) {
            if !fields.iter().any(|(existing, _)| *existing == k) {
                fields.push((k, v));
            }
        }
    }
    fields
}

fn scalar_text(node: &Node) -> String {
    match node {
        Node::Scalar { value, .. } => value.clone(),
        _ => String::new(),
    }
}

fn check_unique_keys(pairs: &[(Node, Node)], errors: &mut Vec<String>) {
    for (i, (ki, _)) in pairs.iter().enumerate() {
        let Node::Scalar { value: vi, .. } = ki else {
            continue;
        };
        for (kj, _) in &pairs[i + 1..] {
            if let Node::Scalar { value: vj, .. } = kj
                && vi == vj
            {
                errors.push(format!(
                    "line {}: mapping key {vi:?} already defined at line {}",
                    kj.line(),
                    ki.line()
                ));
            }
        }
    }
}

fn decode_string(node: &Node, errors: &mut Vec<String>) -> String {
    if node.is_null() {
        return String::new();
    }
    match node {
        Node::Scalar { value, .. } => value.clone(),
        other => {
            errors.push(format!(
                "line {}: cannot unmarshal {} into string",
                other.line(),
                other.describe()
            ));
            String::new()
        }
    }
}

fn decode_bangs(node: &Node, errors: &mut Vec<String>) -> Vec<BangConfig> {
    if node.is_null() {
        return Vec::new();
    }
    let Node::Seq(items, _) = node else {
        errors.push(format!(
            "line {}: cannot unmarshal {} into []main.BangConfig",
            node.line(),
            node.describe()
        ));
        return Vec::new();
    };
    items
        .iter()
        .map(|item| {
            let mut bc = BangConfig::default();
            if item.is_null() {
                return bc;
            }
            for (key, value) in struct_fields(item, "main.BangConfig", errors) {
                let field = match key.as_str() {
                    "trigger" => &mut bc.trigger,
                    "name" => &mut bc.name,
                    "url_template" => &mut bc.url_template,
                    "home_url" => &mut bc.home_url,
                    "suggest_url_template" => &mut bc.suggest_url_template,
                    _ => continue,
                };
                *field = decode_string(value, errors);
            }
            bc
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SINGLE: &str = r#"
default_bang: g
bangs:
  - trigger: g
    name: Google
    url_template: "https://www.google.com/search?q={{{s}}}"
"#;

    #[test]
    fn loads_single_bang() {
        let c = Config::from_yaml(SINGLE.as_bytes()).unwrap();
        assert_eq!(c.default_bang, "g");
        assert_eq!(c.bangs.len(), 1);
        assert_eq!(
            c.bangs[0].url_template,
            "https://www.google.com/search?q={{{s}}}"
        );
    }

    #[test]
    fn loads_multiple_bangs_and_suggest_template() {
        let yaml = r#"
default_bang: g
bangs:
  - trigger: g
    name: Google
    url_template: "https://www.google.com/search?q={{{s}}}"
    suggest_url_template: "https://suggestqueries.google.com/complete/search?client=firefox&q={{{s}}}"
  - trigger: w
    name: Wikipedia
    url_template: "https://en.wikipedia.org/wiki/Special:Search?search={{{s}}}"
  - trigger: yt
    name: YouTube
    url_template: "https://www.youtube.com/results?search_query={{{s}}}"
"#;
        let c = Config::from_yaml(yaml.as_bytes()).unwrap();
        assert_eq!(c.bangs.len(), 3);
        let repo = c.to_repository();
        assert_eq!(
            repo.find_by_trigger(b"g").unwrap().suggest_url_template,
            "https://suggestqueries.google.com/complete/search?client=firefox&q={{{s}}}"
        );
    }

    #[test]
    fn rejects_invalid_configs() {
        let cases = [
            "invalid: yaml: : format",
            "default_bang: g\nbangs:\n  - name: Google\n    url_template: \"https://www.google.com/search?q={{{s}}}\"\n",
            "default_bang: g\nbangs:\n  - trigger: g\n    name: Google\n",
            "default_bang: g\nbangs:\n  - trigger: g\n    url_template: \"not-a-valid-url\"\n",
            "bangs:\n  - trigger: g\n    url_template: \"https://g.com/?q={{{s}}}\"\n    suggest_url_template: \"not-a-url\"\n",
            "bangs: nope\n",
            "default_bang: [a]\n",
            "default_bang: a\ndefault_bang: b\n",
            "- a\n- b\n",
            "bangs:\n  - 5\n",
        ];
        for yaml in cases {
            assert!(
                Config::from_yaml(yaml.as_bytes()).is_err(),
                "expected error for {yaml:?}"
            );
        }
    }

    #[test]
    fn error_messages_match_go() {
        let err = Config::from_yaml(b"bangs:\n  - name: x\n").unwrap_err();
        assert_eq!(err, "bang[0]: trigger is required");
        let err = Config::from_yaml(b"bangs:\n  - trigger: g\n").unwrap_err();
        assert_eq!(err, "bang[g]: url_template is required");
        let err = Config::from_yaml(b"bangs:\n  - trigger: g\n    url_template: x\n").unwrap_err();
        assert_eq!(
            err,
            "bang[g]: url_template must be a valid URL with scheme and host"
        );
    }

    #[test]
    fn scalars_decode_as_literal_text() {
        let yaml = "default_bang: 0x10\nbangs:\n  - trigger: 1.50\n    name: true\n    url_template: https://x.com/?q={{{s}}}\n    home_url: ~\n";
        let c = Config::from_yaml(yaml.as_bytes()).unwrap();
        assert_eq!(c.default_bang, "0x10");
        assert_eq!(c.bangs[0].trigger, "1.50");
        assert_eq!(c.bangs[0].name, "true");
        assert_eq!(c.bangs[0].home_url, "");
    }

    #[test]
    fn empty_and_null_documents_are_empty_configs() {
        assert_eq!(Config::from_yaml(b"").unwrap(), Config::default());
        assert_eq!(Config::from_yaml(b"~\n").unwrap(), Config::default());
        assert_eq!(Config::from_yaml(b"bangs:\n").unwrap(), Config::default());
    }

    #[test]
    fn anchors_merges_and_unknown_keys() {
        let yaml = r#"
extra: {ignored: [1, 2]}
base: &base
  name: Base
  url_template: "https://base.com/?q={{{s}}}"
bangs:
  - <<: *base
    trigger: b
  - <<: *base
    trigger: c
    name: Custom
default_bang: b
---
this: [is, not, read
"#;
        let c = Config::from_yaml(yaml.as_bytes()).unwrap();
        assert_eq!(c.bangs[0].name, "Base");
        assert_eq!(c.bangs[1].name, "Custom");
        assert_eq!(c.bangs[1].url_template, "https://base.com/?q={{{s}}}");
    }

    #[test]
    fn missing_file_is_an_error() {
        let err = Config::from_file("/nonexistent/path/bangs.yaml").unwrap_err();
        assert_eq!(
            err,
            "failed to read config file: open /nonexistent/path/bangs.yaml: no such file or directory"
        );
    }
}
