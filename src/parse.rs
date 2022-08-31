// TODO: remove
#![allow(warnings)]

use crate::expr::*;

use pest::iterators::{Pair, Pairs};
use serde_json::Value;

#[derive(pest_derive::Parser)]
#[grammar = "grammar.pest"]
struct QueryParser {}

pub(crate) fn parse_str(input: &str) -> Expr {
    let mut parser = QueryParser {};
    parser.parse_str(input)
}

impl QueryParser {
    fn parse_str(&mut self, input: &str) -> Expr {
        use pest::Parser;

        let mut pairs = QueryParser::parse(Rule::main, input).unwrap();

        let root = next(&mut pairs);
        expect_rule(&root, Rule::main);

        let expr = next(&mut root.into_inner());
        expect_rule(&expr, Rule::expr);

        self.parse_expr(expr)
    }

    fn parse_expr(&mut self, pair: Pair<Rule>) -> Expr {
        let mut inner = pair.into_inner();
        let mut ret = self.parse_unary(next(&mut inner));

        loop {
            match (inner.next(), inner.next()) {
                (Some(op), Some(right)) => {
                    let right = Box::new(self.parse_unary(right));
                    ret = match op.as_str() {
                        "||" => Expr::Or(Or {
                            left: Box::new(ret),
                            right,
                        }),
                        "&&" => Expr::And(And {
                            left: Box::new(ret),
                            right,
                        }),
                        _ => todo!("unhandled operand {:#?}", op),
                    };
                }
                (None, None) => break,
                expr => panic!("could not parse expression; {:#?}", expr),
            }
        }

        ret
    }

    fn parse_unary(&mut self, pair: Pair<Rule>) -> Expr {
        let pair = next(&mut pair.into_inner());

        match pair.as_rule() {
            Rule::func => self.parse_func(pair),
            Rule::not => self.parse_not(pair),
            Rule::path => self.parse_path(pair),
            Rule::comp => self.parse_comp(pair),
            Rule::paren => self.parse_paren(pair),
            _ => todo!("{:?}", pair.as_rule()),
        }
    }

    fn parse_comp(&mut self, pair: Pair<Rule>) -> Expr {
        let mut inner = pair.into_inner();
        let mut ret = self.parse_val(next(&mut inner));

        loop {
            match (inner.next(), inner.next()) {
                (Some(op), Some(right)) => {
                    let right = Box::new(self.parse_val(right));
                    ret = match op.as_str() {
                        "==" => Expr::Eq {
                            left: Box::new(ret),
                            right,
                        },
                        _ => todo!("unhandled operand {:#?}", op),
                    };
                }
                (None, None) => break,
                expr => todo!("could not parse expression; {:#?}", expr),
            }
        }

        ret
    }

    fn parse_val(&mut self, pair: Pair<Rule>) -> Expr {
        let pair = next(&mut pair.into_inner());

        match pair.as_rule() {
            Rule::func => self.parse_func(pair),
            Rule::path => self.parse_path(pair),
            Rule::kw_log => Expr::Type(Type::Log),
            Rule::kw_send => Expr::Type(Type::Send),
            Rule::kw_receive => Expr::Type(Type::Receive),
            Rule::kw_host => Expr::Host,
            Rule::kw_version => Expr::Version,
            Rule::string => {
                let inner = pair.as_str();
                let end = inner.len() - 1;
                Expr::Value(Value::String(inner[1..end].to_string()))
            }
            Rule::number => {
                let i: i64 = pair.as_str().parse().unwrap();
                Expr::Value(Value::Number(i.into()))
            }
            _ => todo!("{:?}", pair.as_rule()),
        }
    }

    fn parse_ty(&mut self, pair: Pair<Rule>) -> Expr {
        todo!("{:#?}", pair);
    }

    fn parse_path(&mut self, pair: Pair<Rule>) -> Expr {
        let field = pair.as_str()[1..].to_string();

        Expr::Path(Path {
            fields: vec![Field(field)],
        })
    }

    fn parse_not(&mut self, pair: Pair<Rule>) -> Expr {
        let mut pairs = pair.into_inner();
        let pair = next(&mut pairs);

        Expr::Not(Not(Box::new(self.parse_expr(pair))))
    }

    fn parse_func(&mut self, pair: Pair<Rule>) -> Expr {
        let mut pairs = pair.into_inner();

        let ident = self.parse_ident(next(&mut pairs));

        match &ident[..] {
            "empty" => self.parse_func_empty(pairs),
            _ => todo!(),
        }
    }

    fn parse_func_empty(&mut self, mut pairs: Pairs<Rule>) -> Expr {
        let pair = next(&mut pairs);
        let arg = self.parse_expr(pair);

        Expr::Func(Func::Empty(Box::new(arg)))
    }

    fn parse_paren(&mut self, pair: Pair<Rule>) -> Expr {
        let pair = next(&mut pair.into_inner());
        self.parse_expr(pair)
    }

    fn parse_ident(&mut self, pair: Pair<Rule>) -> String {
        pair.as_str().into()
    }
}

fn expect_rule(pair: &Pair<Rule>, rule: Rule) {
    assert!(pair.as_rule() == rule);
}

fn next<'a>(pairs: &mut Pairs<'a, Rule>) -> Pair<'a, Rule> {
    pairs.next().unwrap()
}
