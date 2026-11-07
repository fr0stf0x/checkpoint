//! Parser tối giản cho định dạng KeyValues (VDF) dạng text của Valve.

#[derive(Debug, Clone)]
pub enum Vdf {
    Str(String),
    Obj(Vec<(String, Vdf)>),
}

impl Vdf {
    pub fn get(&self, key: &str) -> Option<&Vdf> {
        match self {
            Vdf::Obj(v) => v.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v),
            Vdf::Str(_) => None,
        }
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Vdf::Str(s) => Some(s.as_str()),
            Vdf::Obj(_) => None,
        }
    }

    pub fn entries(&self) -> &[(String, Vdf)] {
        match self {
            Vdf::Obj(v) => v,
            Vdf::Str(_) => &[],
        }
    }
}

enum Tok {
    S(String),
    Open,
    Close,
}

fn tokenize(s: &str) -> Vec<Tok> {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() || ch == '\u{feff}' {
            i += 1;
            continue;
        }
        if ch == '/' && i + 1 < c.len() && c[i + 1] == '/' {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        match ch {
            '{' => {
                out.push(Tok::Open);
                i += 1;
            }
            '}' => {
                out.push(Tok::Close);
                i += 1;
            }
            '"' => {
                i += 1;
                let mut buf = String::new();
                while i < c.len() && c[i] != '"' {
                    if c[i] == '\\' && i + 1 < c.len() {
                        i += 1;
                        match c[i] {
                            'n' => buf.push('\n'),
                            't' => buf.push('\t'),
                            other => buf.push(other),
                        }
                    } else {
                        buf.push(c[i]);
                    }
                    i += 1;
                }
                i += 1;
                out.push(Tok::S(buf));
            }
            _ => {
                let mut buf = String::new();
                while i < c.len() && !c[i].is_whitespace() && !matches!(c[i], '{' | '}' | '"') {
                    buf.push(c[i]);
                    i += 1;
                }
                // Bỏ qua điều kiện kiểu [$WIN32]
                if !buf.starts_with('[') {
                    out.push(Tok::S(buf));
                }
            }
        }
    }
    out
}

fn parse_obj(toks: &[Tok], i: &mut usize) -> Vec<(String, Vdf)> {
    let mut out = Vec::new();
    while *i < toks.len() {
        match &toks[*i] {
            Tok::Close => {
                *i += 1;
                return out;
            }
            Tok::Open => {
                *i += 1;
            }
            Tok::S(key) => {
                let key = key.clone();
                *i += 1;
                match toks.get(*i) {
                    Some(Tok::Open) => {
                        *i += 1;
                        out.push((key, Vdf::Obj(parse_obj(toks, i))));
                    }
                    Some(Tok::S(val)) => {
                        out.push((key, Vdf::Str(val.clone())));
                        *i += 1;
                    }
                    _ => return out,
                }
            }
        }
    }
    out
}

pub fn parse(s: &str) -> Vdf {
    let toks = tokenize(s);
    let mut i = 0;
    Vdf::Obj(parse_obj(&toks, &mut i))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_libraryfolders() {
        let s = r#""libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"apps" { "228980" "1" }
	}
}"#;
        let v = parse(s);
        let lf = v.get("libraryfolders").unwrap();
        let first = &lf.entries()[0].1;
        assert_eq!(first.str("path"), Some("C:\\Program Files (x86)\\Steam"));
        assert_eq!(first.get("apps").unwrap().str("228980"), Some("1"));
    }
}
