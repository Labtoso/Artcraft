//! Parameter expressions such as `width / 2 + 5 mm` or `30 deg`.
//!
//! Lengths evaluate to millimetres, angles to degrees. Supported: numbers, `+ - * / ^`,
//! parentheses, unary minus, parameter names, unit suffixes (`mm cm m in ft deg rad`) and the
//! functions `sqrt abs sin cos tan min max`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An expression kept as text, so documents stay readable and editable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Expr(pub String);

impl Expr {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn num(v: f64) -> Self {
        Self(format_number(v))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Expr {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

pub fn format_number(v: f64) -> String {
    if v.fract().abs() < 1e-9 && v.abs() < 1e15 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum ExprError {
    #[error("unexpected '{0}' in expression")]
    Unexpected(String),
    #[error("unknown parameter '{0}'")]
    Unknown(String),
    #[error("parameter '{0}' refers to itself")]
    Cycle(String),
    #[error("the expression is empty")]
    Empty,
    #[error("the result is not a finite number")]
    NotFinite,
    #[error("the expression is too long or too deeply nested")]
    TooComplex,
}

const MAX_LEN: usize = 4096;
const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, Default)]
pub struct Params {
    defs: HashMap<String, Expr>,
}

impl Params {
    pub fn define(&mut self, name: &str, e: Expr) {
        self.defs.insert(name.to_owned(), e);
    }
    pub fn eval(&self, e: &Expr) -> Result<f64, ExprError> {
        self.eval_str(e.as_str(), &mut Vec::new())
    }
    pub fn eval_str(&self, s: &str, stack: &mut Vec<String>) -> Result<f64, ExprError> {
        if s.len() > MAX_LEN || stack.len() > MAX_DEPTH {
            return Err(ExprError::TooComplex);
        }
        let tokens = lex(s)?;
        if tokens.is_empty() {
            return Err(ExprError::Empty);
        }
        let mut p = Parser { t: &tokens, i: 0, params: self, stack, depth: 0 };
        let v = p.sum()?;
        if let Some(t) = p.t.get(p.i) {
            return Err(ExprError::Unexpected(t.text()));
        }
        if !v.is_finite() {
            return Err(ExprError::NotFinite);
        }
        Ok(v)
    }
    fn lookup(&self, name: &str, stack: &mut Vec<String>) -> Result<f64, ExprError> {
        let Some(e) = self.defs.get(name) else { return Err(ExprError::Unknown(name.to_owned())) };
        if stack.iter().any(|n| n == name) {
            return Err(ExprError::Cycle(name.to_owned()));
        }
        stack.push(name.to_owned());
        let r = self.eval_str(e.as_str(), stack);
        stack.pop();
        r
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(char),
}

impl Tok {
    fn text(&self) -> String {
        match self {
            Tok::Num(v) => format_number(*v),
            Tok::Ident(s) => s.clone(),
            Tok::Op(c) => c.to_string(),
        }
    }
}

fn lex(s: &str) -> Result<Vec<Tok>, ExprError> {
    let mut out = Vec::new();
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    while let Some(&c) = cs.get(i) {
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || c == '.' || c == ',' {
            let st = i;
            while cs.get(i).is_some_and(|c| c.is_ascii_digit() || *c == '.' || *c == ',') {
                i += 1;
            }
            // Exponent.
            if cs.get(i).is_some_and(|c| *c == 'e' || *c == 'E')
                && cs.get(i + 1).is_some_and(|c| c.is_ascii_digit() || *c == '-' || *c == '+')
            {
                i += 2;
                while cs.get(i).is_some_and(char::is_ascii_digit) {
                    i += 1;
                }
            }
            let text: String = cs.get(st..i).unwrap_or_default().iter().collect::<String>().replace(',', ".");
            let v: f64 = text.parse().map_err(|_| ExprError::Unexpected(text.clone()))?;
            out.push(Tok::Num(v));
        } else if c.is_alphabetic() || c == '_' {
            let st = i;
            while cs.get(i).is_some_and(|c| c.is_alphanumeric() || *c == '_') {
                i += 1;
            }
            out.push(Tok::Ident(cs.get(st..i).unwrap_or_default().iter().collect()));
        } else if "+-*/^()".contains(c) {
            out.push(Tok::Op(c));
            i += 1;
        } else if c == '°' {
            out.push(Tok::Ident("deg".into()));
            i += 1;
        } else {
            return Err(ExprError::Unexpected(c.to_string()));
        }
    }
    Ok(out)
}

fn unit(name: &str) -> Option<f64> {
    Some(match name {
        "mm" => 1.0,
        "cm" => 10.0,
        "m" => 1000.0,
        "in" => 25.4,
        "ft" => 304.8,
        "deg" => 1.0,
        "rad" => 180.0 / std::f64::consts::PI,
        _ => return None,
    })
}

struct Parser<'a> {
    t: &'a [Tok],
    i: usize,
    params: &'a Params,
    stack: &'a mut Vec<String>,
    depth: usize,
}

impl Parser<'_> {
    fn peek_op(&self) -> Option<char> {
        match self.t.get(self.i) {
            Some(Tok::Op(c)) => Some(*c),
            _ => None,
        }
    }
    fn sum(&mut self) -> Result<f64, ExprError> {
        let mut v = self.product()?;
        while let Some(op @ ('+' | '-')) = self.peek_op() {
            self.i += 1;
            let r = self.product()?;
            v = if op == '+' { v + r } else { v - r };
        }
        Ok(v)
    }
    fn product(&mut self) -> Result<f64, ExprError> {
        let mut v = self.unary()?;
        while let Some(op @ ('*' | '/')) = self.peek_op() {
            self.i += 1;
            let r = self.unary()?;
            v = if op == '*' { v * r } else { v / r };
        }
        Ok(v)
    }
    fn unary(&mut self) -> Result<f64, ExprError> {
        match self.peek_op() {
            Some('-') => {
                self.i += 1;
                Ok(-self.unary()?)
            }
            Some('+') => {
                self.i += 1;
                self.unary()
            }
            _ => self.power(),
        }
    }
    fn power(&mut self) -> Result<f64, ExprError> {
        let b = self.postfix()?;
        if self.peek_op() == Some('^') {
            self.i += 1;
            let e = self.unary()?;
            return Ok(b.powf(e));
        }
        Ok(b)
    }
    /// An atom with an optional unit suffix.
    fn postfix(&mut self) -> Result<f64, ExprError> {
        let v = self.atom()?;
        if let Some(Tok::Ident(n)) = self.t.get(self.i)
            && let Some(f) = unit(n) {
                self.i += 1;
                return Ok(v * f);
            }
        Ok(v)
    }
    fn atom(&mut self) -> Result<f64, ExprError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH * 4 {
            return Err(ExprError::TooComplex);
        }
        let tok = self.t.get(self.i).cloned();
        self.i += 1;
        match tok {
            Some(Tok::Num(v)) => Ok(v),
            Some(Tok::Op('(')) => {
                let v = self.sum()?;
                if self.peek_op() != Some(')') {
                    return Err(ExprError::Unexpected("missing )".into()));
                }
                self.i += 1;
                Ok(v)
            }
            Some(Tok::Ident(name)) => {
                if self.peek_op() == Some('(') {
                    self.i += 1;
                    let a = self.sum()?;
                    if self.peek_op() != Some(')') {
                        return Err(ExprError::Unexpected("missing )".into()));
                    }
                    self.i += 1;
                    return match name.as_str() {
                        "sqrt" => Ok(a.sqrt()),
                        "abs" => Ok(a.abs()),
                        "sin" => Ok(a.to_radians().sin()),
                        "cos" => Ok(a.to_radians().cos()),
                        "tan" => Ok(a.to_radians().tan()),
                        _ => Err(ExprError::Unknown(name)),
                    };
                }
                if name == "pi" || name == "PI" {
                    return Ok(std::f64::consts::PI);
                }
                if let Some(u) = unit(&name) {
                    // A bare unit such as "mm" means 1 of it.
                    return Ok(u);
                }
                self.params.lookup(&name, self.stack)
            }
            Some(t) => Err(ExprError::Unexpected(t.text())),
            None => Err(ExprError::Unexpected("end of expression".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates() {
        let mut p = Params::default();
        p.define("width", Expr::new("40 mm"));
        p.define("half", Expr::new("width / 2"));
        let e = |s: &str| p.eval(&Expr::new(s));
        assert_eq!(e("1 + 2 * 3"), Ok(7.0));
        assert_eq!(e("2 cm"), Ok(20.0));
        assert_eq!(e("half + 5"), Ok(25.0));
        assert_eq!(e("-(1+1)^2"), Ok(-4.0));
        assert_eq!(e("2,5"), Ok(2.5));
        assert_eq!(e("sqrt(16)"), Ok(4.0));
        assert!(matches!(e("nope"), Err(ExprError::Unknown(_))));
        assert!(e("1 +").is_err());
        assert!(matches!(e("1/0"), Err(ExprError::NotFinite)));
        p.define("a", Expr::new("b"));
        p.define("b", Expr::new("a"));
        assert!(matches!(p.eval(&Expr::new("a")), Err(ExprError::Cycle(_))));
        let deep = "(".repeat(5000);
        assert!(p.eval(&Expr::new(deep)).is_err());
    }
}
