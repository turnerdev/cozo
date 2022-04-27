use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter, Write};
use lazy_static::lazy_static;
use pest::prec_climber::{Assoc, PrecClimber, Operator};
use ordered_float::OrderedFloat;
use pest::iterators::Pair;
use uuid::Uuid;
use crate::parser::Rule;
use crate::error::Result;
use crate::parser::number::parse_int;
use crate::parser::text_identifier::parse_string;


#[repr(u8)]
#[derive(Ord, PartialOrd, Eq, PartialEq)]
pub enum Tag {
    BoolFalse = 1,
    Null = 2,
    BoolTrue = 3,
    Int = 4,
    Float = 5,
    Text = 6,
    Uuid = 7,
    UInt = 8,

    List = 128,
    Dict = 129,

    Variable = 253,
    Apply = 254,
    MaxTag = 255,
}

impl TryFrom<u8> for Tag {
    type Error = u8;
    #[inline]
    fn try_from(u: u8) -> std::result::Result<Tag, u8> {
        use self::Tag::*;
        Ok(match u {
            1 => BoolFalse,
            2 => Null,
            3 => BoolTrue,
            4 => Int,
            5 => Float,
            6 => Text,
            7 => Uuid,
            8 => UInt,
            128 => List,
            129 => Dict,
            253 => Variable,
            254 => Apply,
            255 => MaxTag,
            v => return Err(v)
        })
    }
}

// Timestamp = 23,
// Datetime = 25,
// Timezone = 27,
// Date = 27,
// Time = 29,
// Duration = 31,
// BigInt = 51,
// BigDecimal = 53,
// Inet = 55,
// Crs = 57,
// BitArr = 60,
// U8Arr = 61,
// I8Arr = 62,
// U16Arr = 63,
// I16Arr = 64,
// U32Arr = 65,
// I32Arr = 66,
// U64Arr = 67,
// I64Arr = 68,
// F16Arr = 69,
// F32Arr = 70,
// F64Arr = 71,
// C32Arr = 72,
// C64Arr = 73,
// C128Arr = 74,


#[derive(Debug, Clone, PartialEq, Ord, PartialOrd, Eq)]
pub enum Value<'a> {
    Null,
    Bool(bool),
    UInt(u64),
    Int(i64),
    Float(OrderedFloat<f64>),
    Uuid(Uuid),
    Text(Cow<'a, str>),
    List(Vec<Value<'a>>),
    Dict(BTreeMap<Cow<'a, str>, Value<'a>>),
    Variable(Cow<'a, str>),
    Apply(Cow<'a, str>, Vec<Value<'a>>),
    EndSentinel,
}

pub type StaticValue = Value<'static>;

impl<'a> Value<'a> {
    #[inline]
    pub fn to_static(self) -> StaticValue {
        match self {
            Value::Null => Value::from(()),
            Value::Bool(b) => Value::from(b),
            Value::UInt(u) => Value::from(u),
            Value::Int(i) => Value::from(i),
            Value::Float(f) => Value::from(f),
            Value::Uuid(u) => Value::from(u),
            Value::Text(t) => Value::from(t.into_owned()),
            Value::Variable(s) => Value::Variable(Cow::Owned(s.into_owned())),
            Value::List(l) => l.into_iter().map(|v| v.to_static()).collect::<Vec<StaticValue>>().into(),
            Value::Apply(op, args) => {
                Value::Apply(Cow::Owned(op.into_owned()),
                             args.into_iter().map(|v| v.to_static()).collect::<Vec<StaticValue>>())
            }
            Value::Dict(d) => d.into_iter()
                .map(|(k, v)| (Cow::Owned(k.into_owned()), v.to_static()))
                .collect::<BTreeMap<Cow<'static, str>, StaticValue>>().into(),
            Value::EndSentinel => panic!("Cannot process sentinel value"),
        }
    }
    #[inline]
    pub fn is_evaluated(&self) -> bool {
        match self {
            Value::Null |
            Value::Bool(_) |
            Value::UInt(_) |
            Value::Int(_) |
            Value::Float(_) |
            Value::Uuid(_) |
            Value::Text(_) |
            Value::EndSentinel => true,
            Value::List(l) => l.iter().all(|v| v.is_evaluated()),
            Value::Dict(d) => d.values().all(|v| v.is_evaluated()),
            Value::Variable(_) => false,
            Value::Apply(_, _) => false
        }
    }
    #[inline]
    pub fn from_pair(pair: pest::iterators::Pair<'a, Rule>) -> Result<Self> {
        PREC_CLIMBER.climb(pair.into_inner(), build_expr_primary, build_expr_infix)
    }
}

impl From<()> for StaticValue {
    #[inline]
    fn from(_: ()) -> Self {
        Value::Null
    }
}

impl From<bool> for StaticValue {
    #[inline]
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<u64> for StaticValue {
    #[inline]
    fn from(u: u64) -> Self {
        Value::UInt(u)
    }
}


impl From<u32> for StaticValue {
    #[inline]
    fn from(u: u32) -> Self {
        Value::UInt(u as u64)
    }
}


impl From<i64> for StaticValue {
    #[inline]
    fn from(i: i64) -> Self {
        Value::Int(i)
    }
}

impl From<i32> for StaticValue {
    #[inline]
    fn from(i: i32) -> Self {
        Value::Int(i as i64)
    }
}

impl From<f64> for StaticValue {
    #[inline]
    fn from(f: f64) -> Self {
        Value::Float(f.into())
    }
}


impl From<OrderedFloat<f64>> for StaticValue {
    #[inline]
    fn from(f: OrderedFloat<f64>) -> Self {
        Value::Float(f)
    }
}

impl<'a> From<&'a str> for Value<'a> {
    #[inline]
    fn from(s: &'a str) -> Self {
        Value::Text(Cow::Borrowed(s))
    }
}

impl From<String> for StaticValue {
    #[inline]
    fn from(s: String) -> Self {
        Value::Text(Cow::Owned(s))
    }
}

impl From<Uuid> for StaticValue {
    #[inline]
    fn from(u: Uuid) -> Self {
        Value::Uuid(u)
    }
}

impl<'a> From<Vec<Value<'a>>> for Value<'a> {
    #[inline]
    fn from(v: Vec<Value<'a>>) -> Self {
        Value::List(v)
    }
}

impl<'a> From<BTreeMap<Cow<'a, str>, Value<'a>>> for Value<'a> {
    #[inline]
    fn from(m: BTreeMap<Cow<'a, str>, Value<'a>>) -> Self {
        Value::Dict(m)
    }
}


impl<'a> Display for Value<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => { f.write_str("null")?; }
            Value::Bool(b) => { f.write_str(if *b { "true" } else { "false" })?; }
            Value::UInt(u) => {
                f.write_str(&u.to_string())?;
                f.write_str("u")?;
            }
            Value::Int(i) => { f.write_str(&i.to_string())?; }
            Value::Float(n) => { f.write_str(&format!("{:e}", n.into_inner()))?; }
            Value::Uuid(u) => { f.write_str(&u.to_string())?; }
            Value::Text(t) => {
                f.write_char('"')?;
                for char in t.chars() {
                    match char {
                        '"' => { f.write_str("\\\"")?; }
                        '\\' => { f.write_str("\\\\")?; }
                        '/' => { f.write_str("\\/")?; }
                        '\x08' => { f.write_str("\\b")?; }
                        '\x0c' => { f.write_str("\\f")?; }
                        '\n' => { f.write_str("\\n")?; }
                        '\r' => { f.write_str("\\r")?; }
                        '\t' => { f.write_str("\\t")?; }
                        c => { f.write_char(c)?; }
                    }
                }
                f.write_char('"')?;
            }
            Value::List(l) => {
                f.write_char('[')?;
                let mut first = true;
                for v in l.iter() {
                    if !first {
                        f.write_char(',')?;
                    }
                    Display::fmt(v, f)?;
                    first = false;
                }
                f.write_char(']')?;
            }
            Value::Dict(d) => {
                f.write_char('{')?;
                let mut first = true;
                for (k, v) in d.iter() {
                    if !first {
                        f.write_char(',')?;
                    }
                    Display::fmt(&Value::Text(k.clone()), f)?;
                    f.write_char(':')?;
                    Display::fmt(v, f)?;
                    first = false;
                }
                f.write_char('}')?;
            }
            Value::Variable(s) => {
                write!(f, "`{}`", s)?
            }
            Value::EndSentinel => {
                write!(f, "Sentinel")?
            }
            Value::Apply(op, args) => {
                write!(f, "({} {})", op,
                       args.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" "))?;
            }
        }
        Ok(())
    }
}

lazy_static! {
    static ref PREC_CLIMBER: PrecClimber<Rule> = {
        use Assoc::*;

        PrecClimber::new(vec![
            Operator::new(Rule::op_or, Left),
            Operator::new(Rule::op_and, Left),
            Operator::new(Rule::op_gt, Left) | Operator::new(Rule::op_lt, Left) | Operator::new(Rule::op_ge,Left) | Operator::new(Rule::op_le, Left),
            Operator::new(Rule::op_mod, Left),
            Operator::new(Rule::op_eq, Left) | Operator::new(Rule::op_ne, Left),
            Operator::new(Rule::op_add, Left) | Operator::new(Rule::op_sub, Left),
            Operator::new(Rule::op_mul, Left) | Operator::new(Rule::op_div, Left),
            Operator::new(Rule::op_pow, Assoc::Right),
            Operator::new(Rule::op_coalesce, Assoc::Left)
        ])
    };
}

pub const OP_ADD: &str = "+";
pub const OP_SUB: &str = "-";
pub const OP_MUL: &str = "*";
pub const OP_DIV: &str = "/";
pub const OP_EQ: &str = "==";
pub const OP_NE: &str = "!=";
pub const OP_OR: &str = "||";
pub const OP_AND: &str = "&&";
pub const OP_MOD: &str = "%";
pub const OP_GT: &str = ">";
pub const OP_GE: &str = ">=";
pub const OP_LT: &str = "<";
pub const OP_LE: &str = "<=";
pub const OP_POW: &str = "**";
pub const OP_COALESCE: &str = "~~";
pub const OP_NEGATE: &str = "!";
pub const OP_MINUS: &str = "--";


fn build_expr_infix<'a>(lhs: Result<Value<'a>>, op: Pair<Rule>, rhs: Result<Value<'a>>) -> Result<Value<'a>> {
    let lhs = lhs?;
    let rhs = rhs?;
    let op = match op.as_rule() {
        Rule::op_add => OP_ADD,
        Rule::op_sub => OP_SUB,
        Rule::op_mul => OP_MUL,
        Rule::op_div => OP_DIV,
        Rule::op_eq => OP_EQ,
        Rule::op_ne => OP_NE,
        Rule::op_or => OP_OR,
        Rule::op_and => OP_AND,
        Rule::op_mod => OP_MOD,
        Rule::op_gt => OP_GT,
        Rule::op_ge => OP_GE,
        Rule::op_lt => OP_LT,
        Rule::op_le => OP_LE,
        Rule::op_pow => OP_POW,
        Rule::op_coalesce => OP_COALESCE,
        _ => unreachable!()
    };
    Ok(Value::Apply(op.into(), vec![lhs, rhs]))
}


fn build_expr_primary(pair: Pair<Rule>) -> Result<Value> {
    match pair.as_rule() {
        Rule::expr => build_expr_primary(pair.into_inner().next().unwrap()),
        Rule::term => build_expr_primary(pair.into_inner().next().unwrap()),
        Rule::grouping => Value::from_pair(pair.into_inner().next().unwrap()),

        Rule::unary => {
            let mut inner = pair.into_inner();
            let op = inner.next().unwrap().as_rule();
            let term = build_expr_primary(inner.next().unwrap())?;
            let op = match op {
                Rule::negate => OP_NEGATE,
                Rule::minus => OP_MINUS,
                _ => unreachable!()
            };
            Ok(Value::Apply(op.into(), vec![term]))
        }

        Rule::pos_int => Ok(Value::Int(pair.as_str().replace('_', "").parse::<i64>()?)),
        Rule::hex_pos_int => Ok(Value::Int(parse_int(pair.as_str(), 16))),
        Rule::octo_pos_int => Ok(Value::Int(parse_int(pair.as_str(), 8))),
        Rule::bin_pos_int => Ok(Value::Int(parse_int(pair.as_str(), 2))),
        Rule::dot_float | Rule::sci_float => Ok(Value::Float(pair.as_str().replace('_', "").parse::<f64>()?.into())),
        Rule::null => Ok(Value::Null),
        Rule::boolean => Ok(Value::Bool(pair.as_str() == "true")),
        Rule::quoted_string | Rule::s_quoted_string | Rule::raw_string => Ok(
            Value::Text(Cow::Owned(parse_string(pair)?))),
        Rule::list => Ok(pair.into_inner().map(|v| build_expr_primary(v)).collect::<Result<Vec<Value>>>()?.into()),
        Rule::dict => {
            Ok(pair.into_inner().map(|p| {
                match p.as_rule() {
                    Rule::dict_pair => {
                        let mut inner = p.into_inner();
                        let name = parse_string(inner.next().unwrap())?;
                        let val = build_expr_primary(inner.next().unwrap())?;
                        Ok((name.into(), val))
                    }
                    _ => todo!()
                }
            }).collect::<Result<BTreeMap<Cow<str>, Value>>>()?.into())
        }
        Rule::param => {
            Ok(Value::Variable(pair.as_str().into()))
        }
        Rule::ident => {
            Ok(Value::Variable(pair.as_str().into()))
        }
        _ => {
            println!("Unhandled rule {:?}", pair.as_rule());
            unimplemented!()
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;
    use pest::Parser as PestParser;

    fn parse_expr_from_str<S: AsRef<str>>(s: S) -> Result<StaticValue> {
        let pair = Parser::parse(Rule::expr, s.as_ref()).unwrap().next().unwrap();
        Value::from_pair(pair).map(|v| v.to_static())
    }

    #[test]
    fn raw_string() {
        println!("{:#?}", parse_expr_from_str(r#####"r#"x"#"#####))
    }

    #[test]
    fn unevaluated() {
        let val = parse_expr_from_str("a+b*c+d").unwrap();
        println!("{}", val);
        assert!(!val.is_evaluated());
    }

    #[test]
    fn parse_literals() {
        assert_eq!(parse_expr_from_str("1").unwrap(), Value::Int(1));
        assert_eq!(parse_expr_from_str("12_3").unwrap(), Value::Int(123));
        assert_eq!(parse_expr_from_str("0xaf").unwrap(), Value::Int(0xaf));
        assert_eq!(parse_expr_from_str("0xafcE_f").unwrap(), Value::Int(0xafcef));
        assert_eq!(parse_expr_from_str("0o1234_567").unwrap(), Value::Int(0o1234567));
        assert_eq!(parse_expr_from_str("0o0001234_567").unwrap(), Value::Int(0o1234567));
        assert_eq!(parse_expr_from_str("0b101010").unwrap(), Value::Int(0b101010));

        assert_eq!(parse_expr_from_str("0.0").unwrap(), Value::Float((0.).into()));
        assert_eq!(parse_expr_from_str("10.022_3").unwrap(), Value::Float(10.0223.into()));
        assert_eq!(parse_expr_from_str("10.022_3e-100").unwrap(), Value::Float(10.0223e-100.into()));

        assert_eq!(parse_expr_from_str("null").unwrap(), Value::Null);
        assert_eq!(parse_expr_from_str("true").unwrap(), Value::Bool(true));
        assert_eq!(parse_expr_from_str("false").unwrap(), Value::Bool(false));
        assert_eq!(parse_expr_from_str(r#""x \n \ty \"""#).unwrap(), Value::Text(Cow::Borrowed("x \n \ty \"")));
        assert_eq!(parse_expr_from_str(r#""x'""#).unwrap(), Value::Text("x'".into()));
        assert_eq!(parse_expr_from_str(r#"'"x"'"#).unwrap(), Value::Text(r##""x""##.into()));
        assert_eq!(parse_expr_from_str(r#####"r###"x"yz"###"#####).unwrap(), (Value::Text(r##"x"yz"##.into())));
    }
}