use crate::*;

use std::fmt;
use std::str::FromStr;

use serde_json::Value;

#[derive(Debug)]
pub(crate) enum Expr {
    Func(Func),
    Not(Not),
    Path(Path),
    Type(Type),
    And(And),
    Or(Or),
    Eq { left: Box<Expr>, right: Box<Expr> },
    Host,
    Version,
    Value(Value),
}

#[derive(Debug)]
pub(crate) enum Func {
    /// Returns `true` if the expression evaluates to an empty list.
    Empty(Box<Expr>),
}

#[derive(Debug)]
pub(crate) struct Not(pub(crate) Box<Expr>);

#[derive(Debug)]
pub(crate) struct And {
    pub(crate) left: Box<Expr>,
    pub(crate) right: Box<Expr>,
}

#[derive(Debug)]
pub(crate) struct Or {
    pub(crate) left: Box<Expr>,
    pub(crate) right: Box<Expr>,
}

/// A path to a message field
#[derive(Debug)]
pub(crate) struct Path {
    pub(crate) fields: Vec<Field>,
}

#[derive(Debug)]
pub(crate) enum Type {
    Log,
    Send,
    Receive,
}

/// References a field in a message.
#[derive(Debug)]
pub(crate) struct Field(pub(crate) String);

impl Expr {
    pub(crate) fn matches<'a>(&'a self, event: &'a Event) -> bool {
        match self {
            Expr::Func(expr) => expr.matches(event),
            Expr::Not(expr) => expr.matches(event),
            Expr::Path(expr) => expr.matches(event),
            Expr::Type(expr) => expr.matches(event),
            Expr::And(expr) => expr.matches(event),
            Expr::Or(expr) => expr.matches(event),
            Expr::Eq { left, right } => {
                if let Expr::Path(path) = &**left {
                    path.eval_eq(event, right.eval(event))
                } else if let Expr::Path(path) = &**right {
                    path.eval_eq(event, left.eval(event))
                } else {
                    left.eval(event) == right.eval(event)
                }
            }
            _ => todo!(),
        }
    }

    pub(crate) fn eval<'a>(&'a self, event: &'a Event) -> &'a Value {
        match self {
            Expr::Host => match event {
                Event::Recv {
                    host: Dot { host, .. },
                    ..
                } => host,
                Event::Send {
                    host: Dot { host, .. },
                    ..
                } => host,
                Event::Log {
                    host: Dot { host, .. },
                    ..
                } => host,
            },
            Expr::Version => match event {
                Event::Recv {
                    host: Dot { version, .. },
                    ..
                } => version,
                Event::Send {
                    host: Dot { version, .. },
                    ..
                } => version,
                Event::Log {
                    host: Dot { version, .. },
                    ..
                } => version,
            },
            Expr::Value(expr) => expr,
            Expr::Path(path) => path.eval(event),
            _ => todo!("{:#?}", self),
        }
    }

    fn is_empty(&self, event: &Event) -> bool {
        match self {
            Expr::Not(_) => false,
            Expr::Path(path) => path.is_empty(event),
            _ => todo!(),
        }
    }
}

impl FromStr for Expr {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Expr> {
        Ok(crate::parse::parse_str(s))
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, _fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl Func {
    fn matches(&self, event: &Event) -> bool {
        match self {
            Func::Empty(expr) => expr.is_empty(event),
        }
    }
}

impl Not {
    fn matches(&self, event: &Event) -> bool {
        !self.0.matches(event)
    }
}

impl And {
    fn matches(&self, event: &Event) -> bool {
        self.left.matches(event) && self.right.matches(event)
    }
}

impl Or {
    fn matches(&self, event: &Event) -> bool {
        self.left.matches(event) || self.right.matches(event)
    }
}

impl Path {
    fn matches(&self, event: &Event) -> bool {
        assert!(self.fields.len() == 1, "only single field supported");

        let message = match event {
            Event::Send { message, .. } => message,
            Event::Recv { message, .. } => message,
            Event::Log { .. } => return false,
        };

        self.fields[0].any(message, &mut |value| !value.is_null())
    }

    pub(crate) fn eval<'a>(&'a self, event: &'a Event) -> &'a Value {
        assert!(self.fields.len() == 1, "only single field supported");

        let message = match event {
            Event::Send { message, .. } => message,
            Event::Recv { message, .. } => message,
            Event::Log { .. } => return &Value::Null,
        };

        if let Some(value) = self.fields[0].eval(message) {
            value
        } else {
            &Value::Null
        }
    }

    pub(crate) fn eval_eq(&self, event: &Event, rhs: &Value) -> bool {
        assert!(self.fields.len() == 1, "only single fields supported");

        let message = match event {
            Event::Send { message, .. } => message,
            Event::Recv { message, .. } => message,
            Event::Log { .. } => return false,
        };

        self.fields[0].any(message, &mut |value| value == rhs)
    }

    fn is_empty(&self, event: &Event) -> bool {
        assert!(self.fields.len() == 1, "only single field supported");

        let message = match event {
            Event::Send { message, .. } => message,
            Event::Recv { message, .. } => message,
            Event::Log { .. } => return false,
        };

        self.fields[0].any(message, &mut |value| {
            value.as_array().map(|v| v.is_empty()).unwrap_or(false)
        })
    }
}

impl Type {
    fn matches(&self, event: &Event) -> bool {
        match (self, event) {
            (Type::Send, Event::Send { .. }) => true,
            (Type::Receive, Event::Recv { .. }) => true,
            (Type::Log, Event::Log { .. }) => true,
            _ => false,
        }
    }
}

impl Field {
    fn any(&self, value: &Value, f: &mut dyn FnMut(&Value) -> bool) -> bool {
        use Value::*;

        match value {
            Null | Bool(_) | Number(_) | String(_) => false,
            Array(v) => v.iter().any(|v| self.any(v, f)),
            Object(v) => v.iter().any(|(k, v)| {
                if *k == self.0 {
                    f(&v) || self.any(v, f)
                } else {
                    self.any(v, f)
                }
            }),
        }
    }

    pub(crate) fn eval<'a>(&'a self, value: &'a Value) -> Option<&'a Value> {
        use Value::*;

        match value {
            Null | Bool(_) | Number(_) | String(_) => None,
            Array(values) => {
                for value in values {
                    if let Some(ret) = self.eval(value) {
                        return Some(ret);
                    }
                }

                None
            }
            Object(v) => {
                for (k, value) in v.iter() {
                    if *k == self.0 {
                        return Some(value);
                    } else if let Some(value) = self.eval(value) {
                        return Some(value);
                    }
                }

                None
            }
        }
    }
}
