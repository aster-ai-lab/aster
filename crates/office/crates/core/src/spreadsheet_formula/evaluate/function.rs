mod aggregate;
mod array;
mod average_if;
mod count_if;
mod criterion;
mod lazy;
mod sum_if;

use aster_use_core::UseResult;

use crate::spreadsheet_formula::registry::BuiltinFunction;
use crate::spreadsheet_formula::SpreadsheetFormulaExpression;
use crate::SpreadsheetFormulaErrorLiteral;

use super::{
    calculation_error, checked_array_cells, finite_or_number_error, scalar_number, EvalValue,
    EvaluationContext, FormulaCellKey, ScalarValue,
};
use aggregate::{
    add_function_cells, aggregate, concatenate, count, count_a, logical_aggregate, logical_not,
    subtotal, text_left, text_len, text_mid, text_right, Aggregate,
};
use array::{row_or_column, sequence, transpose};
use average_if::average_if;
use count_if::count_if;
use lazy::{evaluate_if, evaluate_if_error};
use sum_if::sum_if;

pub(super) fn evaluate_function(
    context: &mut EvaluationContext<'_>,
    name: &str,
    arguments: &[Option<SpreadsheetFormulaExpression>],
    current: FormulaCellKey,
) -> UseResult<EvalValue> {
    let Some(definition) = context.registry.get(name) else {
        return Err(calculation_error(
            "use.office.spreadsheet_formula_function_unsupported",
            format!("Native calculation does not implement function '{name}'."),
        )
        .with_detail("function", name)
        .with_suggestion(
            "Use a closed-registry function listed on recalculate-spreadsheet-formulas. Do not calculate the formula outside the native engine.",
        ));
    };
    if arguments.len() < definition.minimum_arguments
        || definition
            .maximum_arguments
            .is_some_and(|maximum| arguments.len() > maximum)
    {
        return Err(calculation_error(
            "use.office.spreadsheet_formula_function_arity",
            format!(
                "Function '{}' accepts {}{} arguments, but received {}.",
                definition.name,
                definition.minimum_arguments,
                definition.maximum_arguments.map_or_else(
                    || " or more".to_string(),
                    |maximum| {
                        if maximum == definition.minimum_arguments {
                            String::new()
                        } else {
                            format!("-{maximum}")
                        }
                    }
                ),
                arguments.len()
            ),
        )
        .with_detail("function", definition.name.clone())
        .with_detail("arguments", arguments.len())
        .with_suggestion(
            "Match the closed-registry argument count for this function. Recalculate in-process; do not pad, omit, or evaluate arguments outside the native engine.",
        ));
    }
    let function = context.registry.function(name).ok_or_else(|| {
        calculation_error(
            "use.office.spreadsheet_formula_function_unsupported",
            format!("Native function registry has no implementation for '{name}'."),
        )
        .with_suggestion(
            "Use a closed-registry function listed on recalculate-spreadsheet-formulas. Do not calculate the formula outside the native engine.",
        )
    })?;
    if matches!(function, BuiltinFunction::If) {
        return evaluate_if(context, arguments, current);
    }
    if matches!(function, BuiltinFunction::IfError) {
        return evaluate_if_error(context, arguments, current);
    }
    let values = evaluate_arguments(context, arguments, current)?;
    match function {
        BuiltinFunction::Sum => aggregate(context, &values, Aggregate::Sum),
        BuiltinFunction::SumIf => sum_if(context, &values),
        BuiltinFunction::CountIf => count_if(context, &values),
        BuiltinFunction::AverageIf => average_if(context, &values),
        BuiltinFunction::Subtotal => subtotal(context, &values),
        BuiltinFunction::Average => aggregate(context, &values, Aggregate::Average),
        BuiltinFunction::Minimum => aggregate(context, &values, Aggregate::Minimum),
        BuiltinFunction::Maximum => aggregate(context, &values, Aggregate::Maximum),
        BuiltinFunction::Count => count(context, &values),
        BuiltinFunction::CountA => count_a(context, &values),
        BuiltinFunction::Absolute => context.map_numeric(argument(&values, 0)?, |number| {
            finite_or_number_error(number.abs())
        }),
        BuiltinFunction::SquareRoot => context.map_numeric(argument(&values, 0)?, |number| {
            if number < 0.0 {
                ScalarValue::Error(SpreadsheetFormulaErrorLiteral::Number)
            } else {
                finite_or_number_error(number.sqrt())
            }
        }),
        BuiltinFunction::Power => numeric_binary(context, &values, |left, right| {
            finite_or_number_error(left.powf(right))
        }),
        BuiltinFunction::Modulo => numeric_binary(context, &values, |left, right| {
            if right == 0.0 {
                ScalarValue::Error(SpreadsheetFormulaErrorLiteral::DivisionByZero)
            } else {
                finite_or_number_error(left - right * (left / right).floor())
            }
        }),
        BuiltinFunction::Round => numeric_binary(context, &values, |number, digits| {
            if !(-308.0..=308.0).contains(&digits) {
                return ScalarValue::Error(SpreadsheetFormulaErrorLiteral::Number);
            }
            let digits = digits.trunc() as i32;
            finite_or_number_error(excel_round(number, digits))
        }),
        BuiltinFunction::And => logical_aggregate(context, &values, true),
        BuiltinFunction::Or => logical_aggregate(context, &values, false),
        BuiltinFunction::Not => logical_not(context, argument(&values, 0)?),
        BuiltinFunction::Concatenate => concatenate(context, &values),
        BuiltinFunction::Left => text_left(context, &values),
        BuiltinFunction::Right => text_right(context, &values),
        BuiltinFunction::Len => text_len(context, &values),
        BuiltinFunction::Mid => text_mid(context, &values),
        BuiltinFunction::Row => row_or_column(context, &values, current, true),
        BuiltinFunction::Column => row_or_column(context, &values, current, false),
        BuiltinFunction::Sequence => sequence(context, &values),
        BuiltinFunction::Transpose => transpose(context, &values),
        BuiltinFunction::Pi => Ok(EvalValue::Scalar(ScalarValue::Number(std::f64::consts::PI))),
        BuiltinFunction::NotAvailable => Ok(EvalValue::Scalar(ScalarValue::Error(
            SpreadsheetFormulaErrorLiteral::NotAvailable,
        ))),
        BuiltinFunction::If | BuiltinFunction::IfError => Err(calculation_error(
            "use.office.spreadsheet_formula_calculation_invalid",
            "Lazy function reached eager dispatch.",
        )),
    }
}

fn evaluate_arguments(
    context: &mut EvaluationContext<'_>,
    arguments: &[Option<SpreadsheetFormulaExpression>],
    current: FormulaCellKey,
) -> UseResult<Vec<EvalValue>> {
    let mut values = Vec::with_capacity(arguments.len());
    let mut materialized_cells = 0_usize;
    for argument in arguments {
        let value = argument.as_ref().map_or_else(
            || Ok(EvalValue::Scalar(ScalarValue::Blank)),
            |argument| context.evaluate_expression(argument, current),
        )?;
        if let EvalValue::Array(array) = &value {
            let cells = checked_array_cells(array.height(), array.width())?;
            add_function_cells(&mut materialized_cells, cells)?;
        }
        values.push(value);
    }
    Ok(values)
}

fn numeric_binary(
    context: &EvaluationContext<'_>,
    arguments: &[EvalValue],
    operation: impl Fn(f64, f64) -> ScalarValue + Copy,
) -> UseResult<EvalValue> {
    let left = context.materialize(argument(arguments, 0)?)?;
    let right = context.materialize(argument(arguments, 1)?)?;
    context.broadcast_binary(left, right, |left, right| {
        let (Ok(left), Ok(right)) = (scalar_number(left), scalar_number(right)) else {
            return Ok(ScalarValue::Error(SpreadsheetFormulaErrorLiteral::Value));
        };
        Ok(operation(left, right))
    })
}

fn argument(arguments: &[EvalValue], index: usize) -> UseResult<EvalValue> {
    arguments.get(index).cloned().ok_or_else(missing_argument)
}

fn missing_argument() -> aster_use_core::UseError {
    calculation_error(
        "use.office.spreadsheet_formula_calculation_invalid",
        "Native formula function dispatch is missing a validated argument.",
    )
}

/// Excel ROUND: half away from zero on the decimal representation, not binary float.
/// `1.005` therefore becomes `1.01` at 2 digits even when `1.005 * 100` is `100.499…`.
pub(crate) fn excel_round(number: f64, digits: i32) -> f64 {
    if !number.is_finite() {
        return number;
    }
    let negative = number.is_sign_negative() && number != 0.0;
    let abs = number.abs();
    let rounded = round_decimal_text(&abs.to_string(), digits).unwrap_or_else(|| {
        let factor = 10_f64.powi(digits);
        if !factor.is_finite() || factor == 0.0 {
            return abs;
        }
        let scaled = abs * factor;
        if scaled.is_sign_negative() {
            (scaled - 0.5).ceil() / factor
        } else {
            (scaled + 0.5).floor() / factor
        }
    });
    if negative {
        -rounded
    } else {
        rounded
    }
}

fn round_decimal_text(text: &str, digits: i32) -> Option<f64> {
    if text.contains(['e', 'E']) {
        return None;
    }
    let (integer, fraction) = match text.split_once('.') {
        Some((integer, fraction)) => (integer, fraction),
        None => (text, ""),
    };
    if integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    if digits >= 0 {
        let keep = digits as usize;
        let mut digits_vec: Vec<u8> = integer.bytes().chain(fraction.bytes()).collect();
        while digits_vec.len() < integer.len() + keep + 1 {
            digits_vec.push(b'0');
        }
        let round_index = integer.len() + keep;
        let round_up = digits_vec
            .get(round_index)
            .is_some_and(|digit| *digit >= b'5');
        digits_vec.truncate(round_index);
        if round_up {
            bump_decimal_digits(&mut digits_vec);
        }
        while digits_vec.len() < keep {
            digits_vec.insert(0, b'0');
        }
        let split = digits_vec.len() - keep;
        let mut out = String::from_utf8(digits_vec[..split].to_vec()).ok()?;
        if out.is_empty() {
            out.push('0');
        }
        if keep > 0 {
            out.push('.');
            out.push_str(std::str::from_utf8(&digits_vec[split..]).ok()?);
        }
        return out.parse().ok();
    }

    // Negative digits: ROUND(1234, -2) → 1200
    let shift = (-digits) as usize;
    if integer.len() <= shift {
        return Some(0.0);
    }
    let mut digits_vec: Vec<u8> = integer.bytes().collect();
    let round_index = digits_vec.len() - shift;
    let round_up = digits_vec
        .get(round_index)
        .is_some_and(|digit| *digit >= b'5');
    for digit in &mut digits_vec[round_index..] {
        *digit = b'0';
    }
    if round_up {
        let mut prefix = digits_vec[..round_index].to_vec();
        bump_decimal_digits(&mut prefix);
        prefix.extend(std::iter::repeat_n(b'0', shift));
        digits_vec = prefix;
    }
    String::from_utf8(digits_vec).ok()?.parse().ok()
}

fn bump_decimal_digits(digits: &mut Vec<u8>) {
    let mut index = digits.len();
    loop {
        if index == 0 {
            digits.insert(0, b'1');
            return;
        }
        index -= 1;
        if digits[index] < b'9' {
            digits[index] += 1;
            return;
        }
        digits[index] = b'0';
    }
}
