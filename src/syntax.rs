use std::collections::BTreeMap;
use num_bigint::BigInt;
use serde::{Serialize,Deserialize};


#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Copy, Clone)]
pub enum BinaryOp {
    // comparisons
    EQ, NE, LT, LE, GT, GE,

    // booleans
    AND, OR,

    // integers
    PLUS, MINUS, TIMES, DIVIDE, MOD,

    // blobs
    CONCAT,

    // dicts
    IN, INDEX,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Copy, Clone)]
pub enum UnaryOp {
    // booleans
    NOT,

    // integers
    NEGATE,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Copy, Clone)]
pub enum TernaryOp {
    IF,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
#[derive(Serialize, Deserialize)]
pub enum Value {
    Int(BigInt),
    Bool(bool),
    Blob(Vec<u8>),
    Dict(BTreeMap<Value, Value>),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Value::Int(i) => { return i.fmt(f); }
            Value::Bool(b) => { return b.fmt(f); }
            Value::Blob(bytes) => {
                match String::from_utf8(bytes.clone()) {
                    Ok(s) => {
                        // TODO: escaping
                        f.write_str("\"")?;
                        f.write_str(&s)?;
                        f.write_str("\"")?;
                    }
                    _ => {
                        f.write_str("???")?; // TODO: non-utf8 blobs
                    }
                }
                return Ok(());
            }
            Value::Dict(mapping) => {
                f.write_str("{")?;
                let mut first = true;
                for (k, v) in mapping.iter() {
                    if first {
                        first = false;
                    } else {
                        f.write_str(", ")?;
                    }
                    k.fmt(f)?;
                    f.write_str(" |-> ")?;
                    v.fmt(f)?;
                }
                f.write_str("}")?;
                return Ok(());
            }
        }
    }
}

pub fn str2blob(s: &str) -> Vec<u8> {
    s.to_string().into_bytes()
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Copy, Clone)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Exp<A> {
    Root(A),
    Name(A, String),
    Literal(A, Value),
    Unary(A, UnaryOp, Box<Exp<A>>),
    Binary(A, BinaryOp, Box<Exp<A>>, Box<Exp<A>>),
    Ternary(A, TernaryOp, Box<Exp<A>>, Box<Exp<A>>, Box<Exp<A>>),
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum LVal<A> {
    Root(A),
    Index(A, Box<LVal<A>>, Box<Exp<A>>),
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Block<A> {
    pub annotation: A,
    pub name: String,
    pub parameters: Vec<String>,
    pub guards: Vec<Exp<A>>,
    pub assignments: Vec<(LVal<A>, Exp<A>)>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Module<A> {
    pub annotation: A,
    pub blocks: Vec<Block<A>>,
}
