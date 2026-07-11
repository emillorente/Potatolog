use std::borrow::Cow;
use std::collections::HashMap;
use std::io::{Error as IoError};

use crate::{Color, Record};
use crate::filters::{Condition, Expression, Operation, View};
use crate::readers::LogReader;

#[derive(Default)]
struct FilterInner {
    variables_last: HashMap<String, String>,
}

pub struct FilteredLogIterator {
    filter: FilterInner,
    reader: Box<dyn LogReader>,
    view: View,
}

impl FilterInner {
    fn set_variable(&mut self, record: &mut Record, key: &str, value: &str) {
        if let Some(existing) = record.variables.iter_mut().find(|(k, _)| k == key) {
            existing.1 = value.to_owned();
        } else {
            record.variables.push((key.to_owned(), value.to_owned()));
        }
        self.variables_last.insert(key.to_owned(), value.to_owned());
    }

    fn evaluate<'a>(&'a self, expression: &'a Expression, record: &'a Record) -> Cow<'a, str> {
        match expression {
            Expression::Record => Cow::Borrowed(&record.text),
            Expression::Var(name) => record
                .variables
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| Cow::Borrowed(v.as_str()))
                .unwrap_or(Cow::Borrowed("")),
            Expression::LastVarValue(name) => self.variables_last.get(name)
                .map(|v| Cow::Borrowed(v.as_str()))
                .unwrap_or(Cow::Borrowed("")),
            Expression::Constant(value) => Cow::Borrowed(value),
        }
    }

    fn apply_operations(
        &mut self,
        record: &mut Record,
        operations: &[Operation],
    ) -> bool {
        for operation in operations {
            match operation {
                Operation::If { condition, then_ops, else_ops } => {
                    match condition {
                        Condition::Match { expression, pattern } => {
                            let value = self.evaluate(expression, record).into_owned();
                            if let Some(m) =  pattern.match_string(&value) {
                                for (k, v) in m {
                                    self.set_variable(record, &k, &v);
                                }
                                if !self.apply_operations(record, then_ops) {
                                    return false;
                                }
                            } else {
                                if !self.apply_operations(record, else_ops) {
                                    return false;
                                }
                            }
                        }
                    }
                }
                Operation::Set { target, expression } => {
                    let value = self.evaluate(expression, record).into_owned();
                    self.set_variable(record, target, &value);
                }
                Operation::ColorBy(expression) => {
                    let value = self.evaluate(expression, record).into_owned();
                    record.color = Color::FromValue { value };
                }
                Operation::SkipRecord => {
                    return false;
                }
            }
        }
        true
    }
}

impl FilteredLogIterator {
    fn try_next(&mut self) -> Result<Option<Record>, IoError> {
        loop {
            let text = match self.reader.read_record()? {
                Some(t) => t,
                None => return Ok(None),
            };
            let mut record = Record::new(text);

            if !self.filter.apply_operations(&mut record, &self.view.operations) {
                continue
            }

            return Ok(Some(record));
        }
    }
}

impl Iterator for FilteredLogIterator {
    type Item = Result<Record, IoError>;

    fn next(&mut self) -> Option<Result<Record, IoError>> {
        match self.try_next() {
            Ok(Some(r)) => Some(Ok(r)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

pub fn process(reader: Box<dyn LogReader>, view: View) -> FilteredLogIterator {
    FilteredLogIterator {
        filter: Default::default(),
        reader,
        view,
    }
}
