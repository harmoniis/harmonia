#[derive(Debug, Clone)]
pub(crate) enum Sexp {
    List(Vec<Sexp>),
    Atom(String),
    String(String),
}

pub(crate) struct Parser<'a> {
    chars: Vec<char>,
    index: usize,
    _raw: &'a str,
}

pub(crate) fn parse_sexp(raw: &str) -> Result<Sexp, String> {
    let mut parser = Parser::new(raw);
    let sexp = parser.parse_expr()?;
    parser.skip_ws();
    if parser.peek().is_some() {
        return Err("unexpected trailing content".to_string());
    }
    Ok(sexp)
}

impl<'a> Parser<'a> {
    fn new(raw: &'a str) -> Self {
        Self {
            chars: raw.chars().collect(),
            index: 0,
            _raw: raw,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.index += 1;
        Some(ch)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(ch) if ch.is_whitespace()) {
            self.index += 1;
        }
    }

    fn parse_expr(&mut self) -> Result<Sexp, String> {
        self.skip_ws();
        match self.peek() {
            Some('(') => self.parse_list(),
            Some('"') => self.parse_string(),
            Some(_) => self.parse_atom(),
            None => Err("unexpected end of input".to_string()),
        }
    }

    fn parse_list(&mut self) -> Result<Sexp, String> {
        self.bump();
        let mut items = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(')') => {
                    self.bump();
                    return Ok(Sexp::List(items));
                }
                Some(_) => items.push(self.parse_expr()?),
                None => return Err("unterminated list".to_string()),
            }
        }
    }

    fn parse_string(&mut self) -> Result<Sexp, String> {
        self.bump();
        let mut out = String::new();
        loop {
            match self.bump() {
                Some('"') => return Ok(Sexp::String(out)),
                Some('\\') => match self.bump() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(other) => out.push(other),
                    None => return Err("unterminated escape".to_string()),
                },
                Some(ch) => out.push(ch),
                None => return Err("unterminated string".to_string()),
            }
        }
    }

    fn parse_atom(&mut self) -> Result<Sexp, String> {
        let mut out = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() || ch == '(' || ch == ')' {
                break;
            }
            out.push(ch);
            self.index += 1;
        }
        if out.is_empty() {
            Err("expected atom".to_string())
        } else {
            Ok(Sexp::Atom(out))
        }
    }
}

pub(crate) fn plist_view(sexp: &Sexp) -> Result<&[Sexp], String> {
    match sexp {
        Sexp::List(items) => {
            if items.is_empty() {
                return Ok(items);
            }
            if let Sexp::Atom(atom) = &items[0] {
                if atom.starts_with(':')
                    && items.len() > 1
                    && matches!(items[1], Sexp::Atom(_))
                    && items[1].atom_starts_with_keyword()
                {
                    return Ok(&items[1..]);
                }
            }
            Ok(items)
        }
        _ => Err("expected plist list".to_string()),
    }
}

pub(crate) trait KeywordAtom {
    fn atom_starts_with_keyword(&self) -> bool;
}

impl KeywordAtom for Sexp {
    fn atom_starts_with_keyword(&self) -> bool {
        matches!(self, Sexp::Atom(atom) if atom.starts_with(':'))
    }
}

pub(crate) fn plist_value<'a>(items: &'a [Sexp], key: &str) -> Option<&'a Sexp> {
    let needle = format!(":{key}");
    let mut index = 0;
    while index + 1 < items.len() {
        if let Sexp::Atom(atom) = &items[index] {
            if atom.eq_ignore_ascii_case(&needle) {
                return items.get(index + 1);
            }
        }
        index += 2;
    }
    None
}

pub(crate) fn plist_list<'a>(items: &'a [Sexp], key: &str) -> Option<&'a [Sexp]> {
    match plist_value(items, key) {
        Some(Sexp::List(list)) => Some(list.as_slice()),
        _ => None,
    }
}

pub(crate) fn plist_f64(items: &[Sexp], key: &str) -> Option<f64> {
    plist_value(items, key).and_then(sexp_to_f64)
}

pub(crate) fn plist_i64(items: &[Sexp], key: &str) -> Option<i64> {
    plist_value(items, key).and_then(sexp_to_i64)
}

pub(crate) fn plist_bool(items: &[Sexp], key: &str) -> Option<bool> {
    plist_value(items, key).and_then(sexp_to_bool)
}

pub(crate) fn plist_string(items: &[Sexp], key: &str) -> Option<String> {
    plist_value(items, key).and_then(sexp_to_string_value)
}

pub(crate) fn sexp_to_f64(sexp: &Sexp) -> Option<f64> {
    match sexp {
        Sexp::Atom(atom) => atom.parse::<f64>().ok(),
        Sexp::String(text) => text.parse::<f64>().ok(),
        Sexp::List(_) => None,
    }
}

pub(crate) fn sexp_to_i64(sexp: &Sexp) -> Option<i64> {
    match sexp {
        Sexp::Atom(atom) => atom
            .parse::<i64>()
            .ok()
            .or_else(|| atom.parse::<f64>().ok().map(|value| value.round() as i64)),
        Sexp::String(text) => text.parse::<i64>().ok(),
        Sexp::List(_) => None,
    }
}

pub(crate) fn sexp_to_bool(sexp: &Sexp) -> Option<bool> {
    match sexp {
        Sexp::Atom(atom) if atom.eq_ignore_ascii_case("t") => Some(true),
        Sexp::Atom(atom) if atom.eq_ignore_ascii_case("nil") => Some(false),
        Sexp::Atom(atom) if atom.eq_ignore_ascii_case(":true") => Some(true),
        Sexp::Atom(atom) if atom.eq_ignore_ascii_case(":false") => Some(false),
        Sexp::String(text) if text.eq_ignore_ascii_case("true") => Some(true),
        Sexp::String(text) if text.eq_ignore_ascii_case("false") => Some(false),
        _ => None,
    }
}

pub(crate) fn sexp_to_string_value(sexp: &Sexp) -> Option<String> {
    match sexp {
        Sexp::Atom(atom) => Some(atom.clone()),
        Sexp::String(text) => Some(text.clone()),
        Sexp::List(_) => None,
    }
}

pub(crate) fn parse_number_list(sexp: &Sexp) -> Result<Vec<f64>, String> {
    match sexp {
        Sexp::List(items) => items
            .iter()
            .map(|item| sexp_to_f64(item).ok_or_else(|| "expected numeric atom".to_string()))
            .collect(),
        _ => Err("expected list".to_string()),
    }
}

pub(crate) fn parse_fixed_array<const N: usize>(
    sexp: Option<&Sexp>,
    label: &str,
) -> Result<[f64; N], String> {
    let values = parse_vector_exact(sexp, N, label)?;
    let mut output = [0.0; N];
    for (slot, value) in output.iter_mut().zip(values.iter()) {
        *slot = *value;
    }
    Ok(output)
}

pub(crate) fn parse_vector_exact(
    sexp: Option<&Sexp>,
    expected: usize,
    label: &str,
) -> Result<Vec<f64>, String> {
    let values = parse_number_list(sexp.ok_or_else(|| format!("missing {label}"))?)?;
    if values.len() != expected {
        return Err(format!(
            "invalid {label}: expected {expected} values, got {}",
            values.len()
        ));
    }
    Ok(values)
}
