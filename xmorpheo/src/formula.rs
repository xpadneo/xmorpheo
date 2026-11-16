// SPDX-License-Identifier: GPL-2.0-or-later

//! Curve formulas.
//!
//! A formula maps the normalized input `x` to the normalized output. The
//! syntax follows the input mixing expressions of Euro Truck Simulator 2
//! (`controls.sii`), restricted to the stateless arithmetic part:
//!
//! ```text
//! pow(normalize(abs(x), 0.05), 1.7) * sign(x)
//! ```
//!
//! Named constants are resolved while parsing. The canonical text of a
//! parsed formula identifies its curve, so formulas that only differ in
//! spacing or constant names share one curve.

use std::collections::HashMap;
use std::fmt;

/// A parsed formula.
#[derive(Clone, Debug, PartialEq)]
pub struct Formula {
    expr: Expr,
}

#[derive(Clone, Debug, PartialEq)]
enum Expr {
    Num(f64),
    /// A variable by index. In a [`Formula`], only `x` with index 0.
    Var(usize),
    Neg(Box<Expr>),
    Bin(Op, Box<Expr>, Box<Expr>),
    Call(Func, Vec<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Func {
    Abs,
    Sign,
    Pow,
    Min,
    Max,
    Clamp,
    Sel,
    Normalize,
    Deadzone,
}

impl Func {
    fn lookup(name: &str) -> Option<(Func, usize)> {
        Some(match name {
            "abs" => (Func::Abs, 1),
            "sign" => (Func::Sign, 1),
            "pow" => (Func::Pow, 2),
            "min" => (Func::Min, 2),
            "max" => (Func::Max, 2),
            "clamp" => (Func::Clamp, 3),
            "sel" => (Func::Sel, 3),
            "normalize" => (Func::Normalize, 2),
            "deadzone" => (Func::Deadzone, 2),
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Func::Abs => "abs",
            Func::Sign => "sign",
            Func::Pow => "pow",
            Func::Min => "min",
            Func::Max => "max",
            Func::Clamp => "clamp",
            Func::Sel => "sel",
            Func::Normalize => "normalize",
            Func::Deadzone => "deadzone",
        }
    }

    fn eval(self, a: &[f64]) -> f64 {
        match self {
            Func::Abs => a[0].abs(),
            Func::Sign => sign(a[0]),
            Func::Pow => a[0].powf(a[1]),
            Func::Min => a[0].min(a[1]),
            Func::Max => a[0].max(a[1]),
            Func::Clamp => a[0].max(a[1]).min(a[2]),
            Func::Sel => {
                if a[0] != 0.0 {
                    a[1]
                } else {
                    a[2]
                }
            }
            // Maps dz..1 to 0..1, everything below dz to 0.
            Func::Normalize => ((a[0] - a[1]) / (1.0 - a[1])).clamp(0.0, 1.0),
            // Like normalize, but for both directions of a centered axis.
            Func::Deadzone => sign(a[0]) * ((a[0].abs() - a[1]) / (1.0 - a[1])).clamp(0.0, 1.0),
        }
    }
}

/// Whether a name is a function of the formula language.
pub fn is_function(name: &str) -> bool {
    Func::lookup(name).is_some()
}

/// Unlike `f64::signum`, the sign of zero is zero.
fn sign(v: f64) -> f64 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// Errors while parsing a formula, with the byte position in the input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    UnexpectedChar {
        pos: usize,
        ch: char,
    },
    UnexpectedEnd,
    UnexpectedToken {
        pos: usize,
    },
    UnknownName {
        pos: usize,
        name: String,
    },
    UnknownFunction {
        pos: usize,
        name: String,
    },
    WrongArity {
        pos: usize,
        name: String,
        expected: usize,
        got: usize,
    },
    /// A part of the formula depends on more than one variable, so it
    /// cannot become a sum of curves in one variable each.
    NotSeparable(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnexpectedChar { pos, ch } => write!(f, "unexpected '{ch}' at {pos}"),
            Error::UnexpectedEnd => write!(f, "unexpected end of formula"),
            Error::UnexpectedToken { pos } => write!(f, "unexpected token at {pos}"),
            Error::UnknownName { pos, name } => write!(f, "unknown name '{name}' at {pos}"),
            Error::UnknownFunction { pos, name } => {
                write!(f, "unknown function '{name}' at {pos}")
            }
            Error::WrongArity {
                pos,
                name,
                expected,
                got,
            } => write!(
                f,
                "'{name}' at {pos} takes {expected} arguments, {got} given"
            ),
            Error::NotSeparable(part) => write!(
                f,
                "'{part}' combines several variables; a formula must be a sum of parts \
                 that each depend on one variable, like 'x - y' or 'pow(x, 2) + y / 2', \
                 see the limits in docs/FORMULAS.md"
            ),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Sym(char),
}

fn tokenize(src: &str) -> Result<Vec<(usize, Tok)>, Error> {
    let mut toks = Vec::new();
    let mut chars = src.char_indices().peekable();
    while let Some(&(pos, ch)) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
        } else if ch.is_ascii_digit() || ch == '.' {
            let mut end = pos;
            while let Some(&(i, c)) = chars.peek() {
                if c.is_ascii_digit() || c == '.' {
                    end = i + c.len_utf8();
                    chars.next();
                } else {
                    break;
                }
            }
            let num = src[pos..end]
                .parse()
                .map_err(|_| Error::UnexpectedChar { pos, ch })?;
            toks.push((pos, Tok::Num(num)));
        } else if ch.is_ascii_alphabetic() || ch == '_' {
            let mut end = pos;
            while let Some(&(i, c)) = chars.peek() {
                if c.is_ascii_alphanumeric() || c == '_' {
                    end = i + 1;
                    chars.next();
                } else {
                    break;
                }
            }
            toks.push((pos, Tok::Ident(src[pos..end].to_string())));
        } else if "+-*/(),".contains(ch) {
            toks.push((pos, Tok::Sym(ch)));
            chars.next();
        } else {
            return Err(Error::UnexpectedChar { pos, ch });
        }
    }
    Ok(toks)
}

struct Parser<'a> {
    toks: Vec<(usize, Tok)>,
    pos: usize,
    constants: &'a HashMap<String, f64>,
    vars: &'a [&'a str],
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|(_, t)| t)
    }

    fn next(&mut self) -> Result<(usize, Tok), Error> {
        let tok = self
            .toks
            .get(self.pos)
            .cloned()
            .ok_or(Error::UnexpectedEnd)?;
        self.pos += 1;
        Ok(tok)
    }

    fn expect(&mut self, sym: char) -> Result<(), Error> {
        match self.next()? {
            (_, Tok::Sym(c)) if c == sym => Ok(()),
            (pos, _) => Err(Error::UnexpectedToken { pos }),
        }
    }

    fn expr(&mut self) -> Result<Expr, Error> {
        let mut lhs = self.term()?;
        while let Some(Tok::Sym(c @ ('+' | '-'))) = self.peek() {
            let op = if *c == '+' { Op::Add } else { Op::Sub };
            self.pos += 1;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(self.term()?));
        }
        Ok(lhs)
    }

    fn term(&mut self) -> Result<Expr, Error> {
        let mut lhs = self.unary()?;
        while let Some(Tok::Sym(c @ ('*' | '/'))) = self.peek() {
            let op = if *c == '*' { Op::Mul } else { Op::Div };
            self.pos += 1;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(self.unary()?));
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Result<Expr, Error> {
        if let Some(Tok::Sym('-')) = self.peek() {
            self.pos += 1;
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, Error> {
        match self.next()? {
            (_, Tok::Num(n)) => Ok(Expr::Num(n)),
            (_, Tok::Sym('(')) => {
                let e = self.expr()?;
                self.expect(')')?;
                Ok(e)
            }
            (pos, Tok::Ident(name)) => {
                if let Some(Tok::Sym('(')) = self.peek() {
                    self.pos += 1;
                    self.call(pos, name)
                } else if let Some(i) = self.vars.iter().position(|v| *v == name) {
                    Ok(Expr::Var(i))
                } else if let Some(v) = self.constants.get(&name) {
                    Ok(Expr::Num(*v))
                } else {
                    Err(Error::UnknownName { pos, name })
                }
            }
            (pos, _) => Err(Error::UnexpectedToken { pos }),
        }
    }

    fn call(&mut self, pos: usize, name: String) -> Result<Expr, Error> {
        let (func, arity) = Func::lookup(&name).ok_or_else(|| Error::UnknownFunction {
            pos,
            name: name.clone(),
        })?;
        let mut args = Vec::new();
        if let Some(Tok::Sym(')')) = self.peek() {
            self.pos += 1;
        } else {
            loop {
                args.push(self.expr()?);
                match self.next()? {
                    (_, Tok::Sym(',')) => continue,
                    (_, Tok::Sym(')')) => break,
                    (pos, _) => return Err(Error::UnexpectedToken { pos }),
                }
            }
        }
        if args.len() != arity {
            return Err(Error::WrongArity {
                pos,
                name,
                expected: arity,
                got: args.len(),
            });
        }
        Ok(Expr::Call(func, args))
    }
}

fn parse(src: &str, constants: &HashMap<String, f64>, vars: &[&str]) -> Result<Expr, Error> {
    let mut p = Parser {
        toks: tokenize(src)?,
        pos: 0,
        constants,
        vars,
    };
    let expr = p.expr()?;
    if p.pos < p.toks.len() {
        return Err(Error::UnexpectedToken {
            pos: p.toks[p.pos].0,
        });
    }
    Ok(expr)
}

impl Formula {
    /// Parses a formula in `x`. Names other than `x` and the function names
    /// are looked up in `constants`.
    pub fn parse(src: &str, constants: &HashMap<String, f64>) -> Result<Self, Error> {
        Ok(Self {
            expr: parse(src, constants, &["x"])?,
        })
    }

    /// Evaluates the formula for the normalized input `x`.
    pub fn eval(&self, x: f64) -> f64 {
        eval(&self.expr, &[x])
    }
}

fn eval(e: &Expr, vars: &[f64]) -> f64 {
    match e {
        Expr::Num(n) => *n,
        Expr::Var(i) => vars[*i],
        Expr::Neg(a) => -eval(a, vars),
        Expr::Bin(op, a, b) => {
            let (a, b) = (eval(a, vars), eval(b, vars));
            match op {
                Op::Add => a + b,
                Op::Sub => a - b,
                Op::Mul => a * b,
                Op::Div => a / b,
            }
        }
        Expr::Call(f, args) => {
            let vals: Vec<f64> = args.iter().map(|a| eval(a, vars)).collect();
            f.eval(&vals)
        }
    }
}

/// A formula in several named variables, like `x - y`.
#[derive(Clone, Debug, PartialEq)]
pub struct MultiFormula {
    expr: Expr,
    vars: Vec<String>,
}

/// A part of a [`MultiFormula`] that depends on one variable: the index of
/// the variable, and the part as a formula in `x`.
#[derive(Clone, Debug, PartialEq)]
pub struct Term {
    pub var: usize,
    pub curve: Formula,
}

/// The variables an expression uses.
fn vars_of(e: &Expr, out: &mut Vec<usize>) {
    match e {
        Expr::Num(_) => {}
        Expr::Var(i) => {
            if !out.contains(i) {
                out.push(*i);
            }
        }
        Expr::Neg(a) => vars_of(a, out),
        Expr::Bin(_, a, b) => {
            vars_of(a, out);
            vars_of(b, out);
        }
        Expr::Call(_, args) => args.iter().for_each(|a| vars_of(a, out)),
    }
}

fn single_var(e: &Expr) -> Option<Option<usize>> {
    let mut vars = Vec::new();
    vars_of(e, &mut vars);
    match vars.len() {
        0 => Some(None),
        1 => Some(Some(vars[0])),
        _ => None,
    }
}

/// Splits an expression into a sum of parts that each use at most one
/// variable.
fn split(e: &Expr, names: &[String], out: &mut Vec<(Option<usize>, Expr)>) -> Result<(), Error> {
    if let Some(var) = single_var(e) {
        out.push((var, e.clone()));
        return Ok(());
    }
    let scale = |out: &mut Vec<(Option<usize>, Expr)>, a: &Expr, op: Op, c: &Expr, left: bool| {
        let mut parts = Vec::new();
        split(a, names, &mut parts)?;
        for (var, p) in parts {
            let (l, r) = if left { (c.clone(), p) } else { (p, c.clone()) };
            out.push((var, Expr::Bin(op, Box::new(l), Box::new(r))));
        }
        Ok::<(), Error>(())
    };
    match e {
        Expr::Neg(a) => {
            let mut parts = Vec::new();
            split(a, names, &mut parts)?;
            out.extend(parts.into_iter().map(|(v, p)| (v, Expr::Neg(Box::new(p)))));
            Ok(())
        }
        Expr::Bin(Op::Add, a, b) => {
            split(a, names, out)?;
            split(b, names, out)
        }
        Expr::Bin(Op::Sub, a, b) => {
            split(a, names, out)?;
            split(&Expr::Neg(b.clone()), names, out)
        }
        Expr::Bin(Op::Mul, a, b) if single_var(a) == Some(None) => scale(out, b, Op::Mul, a, true),
        Expr::Bin(Op::Mul, a, b) if single_var(b) == Some(None) => scale(out, a, Op::Mul, b, false),
        Expr::Bin(Op::Div, a, b) if single_var(b) == Some(None) => scale(out, a, Op::Div, b, false),
        _ => Err(Error::NotSeparable(Named(e, names).to_string())),
    }
}

/// Renames the only variable of an expression to `x`.
fn to_x(e: Expr) -> Expr {
    match e {
        Expr::Var(_) => Expr::Var(0),
        Expr::Neg(a) => Expr::Neg(Box::new(to_x(*a))),
        Expr::Bin(op, a, b) => Expr::Bin(op, Box::new(to_x(*a)), Box::new(to_x(*b))),
        Expr::Call(f, args) => Expr::Call(f, args.into_iter().map(to_x).collect()),
        e => e,
    }
}

impl MultiFormula {
    /// Parses a formula in the variables `vars`. Other names are looked up
    /// in `constants`.
    pub fn parse(
        src: &str,
        constants: &HashMap<String, f64>,
        vars: &[&str],
    ) -> Result<Self, Error> {
        Ok(Self {
            expr: parse(src, constants, vars)?,
            vars: vars.iter().map(|v| v.to_string()).collect(),
        })
    }

    /// Evaluates the formula for normalized values of its variables.
    pub fn eval(&self, values: &[f64]) -> f64 {
        eval(&self.expr, values)
    }

    /// Whether the formula uses each variable.
    pub fn used(&self) -> Vec<bool> {
        let mut vars = Vec::new();
        vars_of(&self.expr, &mut vars);
        (0..self.vars.len()).map(|i| vars.contains(&i)).collect()
    }

    /// Splits the formula into a sum of terms in one variable each, at most
    /// one term per variable. Parts without a variable join the term of the
    /// first variable that has one, or of the first variable if none has.
    pub fn terms(&self) -> Result<Vec<Term>, Error> {
        let mut parts = Vec::new();
        split(&self.expr, &self.vars, &mut parts)?;
        let mut sums: Vec<(usize, Expr)> = Vec::new();
        let mut constant: Option<Expr> = None;
        for (var, part) in parts {
            let add = |acc: Option<Expr>| match acc {
                Some(a) => Expr::Bin(Op::Add, Box::new(a), Box::new(part.clone())),
                None => part.clone(),
            };
            match var {
                Some(v) => match sums.iter_mut().find(|(w, _)| *w == v) {
                    Some((_, e)) => *e = add(Some(e.clone())),
                    None => sums.push((v, part)),
                },
                None => constant = Some(add(constant.take())),
            }
        }
        if let Some(c) = constant {
            match sums.iter_mut().min_by_key(|(v, _)| *v) {
                Some((_, e)) => *e = Expr::Bin(Op::Add, Box::new(e.clone()), Box::new(c)),
                None => sums.push((0, c)),
            }
        }
        sums.sort_by_key(|(v, _)| *v);
        Ok(sums
            .into_iter()
            .map(|(var, e)| Term {
                var,
                curve: Formula { expr: to_x(e) },
            })
            .collect())
    }
}

/// An expression printed with the names of its variables.
struct Named<'a>(&'a Expr, &'a [String]);

impl fmt::Display for Named<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_expr(self.0, self.1, f)
    }
}

impl fmt::Display for MultiFormula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Named(&self.expr, &self.vars))
    }
}

/// The canonical text: fully parenthesized, constants replaced by their
/// values. Equal canonical text means equal curves.
impl fmt::Display for Formula {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.expr)
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_expr(self, &["x".to_string()], f)
    }
}

fn write_expr(e: &Expr, names: &[String], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let sub = |e| Named(e, names);
    match e {
        Expr::Num(n) => write!(f, "{n:?}"),
        Expr::Var(i) => match names.get(*i) {
            Some(name) => write!(f, "{name}"),
            None => write!(f, "?"),
        },
        Expr::Neg(a) => write!(f, "(-{})", sub(a)),
        Expr::Bin(op, a, b) => {
            let op = match op {
                Op::Add => '+',
                Op::Sub => '-',
                Op::Mul => '*',
                Op::Div => '/',
            };
            write!(f, "({}{op}{})", sub(a), sub(b))
        }
        Expr::Call(func, args) => {
            write!(f, "{}(", func.name())?;
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    write!(f, ",")?;
                }
                write!(f, "{}", sub(a))?;
            }
            write!(f, ")")
        }
    }
}
