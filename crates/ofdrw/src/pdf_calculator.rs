//! Bounded PDF Type 4 calculator expressions (ISO 32000-1, 7.10.5 and Annex B).

use crate::OfdError;

const MAX_CODE_BYTES: usize = 65_536;
const MAX_INSTRUCTIONS: usize = 4096;
const MAX_DEPTH: usize = 32;
const MAX_STACK: usize = 256;

fn invalid(reason: &str) -> OfdError {
    OfdError::Invalid(format!("invalid PDF calculator Function: {reason}"))
}

fn limit(reason: &str) -> OfdError {
    OfdError::Limit(format!("PDF calculator Function {reason}"))
}

#[derive(Clone, Copy, Debug)]
enum Value {
    Integer(i32),
    Real(f64),
    Boolean(bool),
}

impl Value {
    fn number(self) -> Result<f64, OfdError> {
        match self {
            Self::Integer(value) => Ok(f64::from(value)),
            Self::Real(value) => Ok(value),
            Self::Boolean(_) => Err(invalid("expected a number")),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Operator {
    Abs,
    Add,
    And,
    Atan,
    Bitshift,
    Ceiling,
    Copy,
    Cos,
    Cvi,
    Cvr,
    Div,
    Dup,
    Eq,
    Exch,
    Exp,
    Floor,
    Ge,
    Gt,
    Idiv,
    Index,
    Le,
    Ln,
    Log,
    Lt,
    Mod,
    Mul,
    Ne,
    Neg,
    Not,
    Or,
    Pop,
    Roll,
    Round,
    Sin,
    Sqrt,
    Sub,
    Truncate,
    Xor,
}

impl Operator {
    fn parse(token: &str) -> Option<Self> {
        Some(match token {
            "abs" => Self::Abs,
            "add" => Self::Add,
            "and" => Self::And,
            "atan" => Self::Atan,
            "bitshift" => Self::Bitshift,
            "ceiling" => Self::Ceiling,
            "copy" => Self::Copy,
            "cos" => Self::Cos,
            "cvi" => Self::Cvi,
            "cvr" => Self::Cvr,
            "div" => Self::Div,
            "dup" => Self::Dup,
            "eq" => Self::Eq,
            "exch" => Self::Exch,
            "exp" => Self::Exp,
            "floor" => Self::Floor,
            "ge" => Self::Ge,
            "gt" => Self::Gt,
            "idiv" => Self::Idiv,
            "index" => Self::Index,
            "le" => Self::Le,
            "ln" => Self::Ln,
            "log" => Self::Log,
            "lt" => Self::Lt,
            "mod" => Self::Mod,
            "mul" => Self::Mul,
            "ne" => Self::Ne,
            "neg" => Self::Neg,
            "not" => Self::Not,
            "or" => Self::Or,
            "pop" => Self::Pop,
            "roll" => Self::Roll,
            "round" => Self::Round,
            "sin" => Self::Sin,
            "sqrt" => Self::Sqrt,
            "sub" => Self::Sub,
            "truncate" => Self::Truncate,
            "xor" => Self::Xor,
            _ => return None,
        })
    }
}

enum Instruction {
    Literal(Value),
    Operator(Operator),
    If(Vec<Node>),
    IfElse(Vec<Node>, Vec<Node>),
}

struct Node {
    id: usize,
    instruction: Instruction,
}

struct Parser<'a> {
    tokens: Vec<&'a [u8]>,
    next: usize,
    instructions: usize,
}

impl Parser<'_> {
    fn take(&mut self) -> Result<&[u8], OfdError> {
        let token = self
            .tokens
            .get(self.next)
            .copied()
            .ok_or_else(|| invalid("unexpected end of code"))?;
        self.next += 1;
        Ok(token)
    }

    fn expect(&mut self, expected: &[u8]) -> Result<(), OfdError> {
        if self.take()? == expected {
            Ok(())
        } else {
            Err(invalid("invalid conditional or block syntax"))
        }
    }

    fn block(&mut self, depth: usize) -> Result<Vec<Node>, OfdError> {
        if depth > MAX_DEPTH {
            return Err(limit("nesting exceeds 32"));
        }
        self.expect(b"{")?;
        let mut nodes = Vec::new();
        while self.tokens.get(self.next) != Some(&b"}".as_slice()) {
            let instruction = if self.tokens.get(self.next) == Some(&b"{".as_slice()) {
                let first = self.block(depth + 1)?;
                if self.tokens.get(self.next) == Some(&b"{".as_slice()) {
                    let second = self.block(depth + 1)?;
                    self.expect(b"ifelse")?;
                    Instruction::IfElse(first, second)
                } else {
                    self.expect(b"if")?;
                    Instruction::If(first)
                }
            } else {
                match self.take()? {
                    b"true" => Instruction::Literal(Value::Boolean(true)),
                    b"false" => Instruction::Literal(Value::Boolean(false)),
                    token => {
                        let token = std::str::from_utf8(token)
                            .map_err(|_| invalid("invalid code token"))?;
                        if let Some(operator) = Operator::parse(token) {
                            Instruction::Operator(operator)
                        } else {
                            Instruction::Literal(parse_number(token)?)
                        }
                    }
                }
            };
            if self.instructions == MAX_INSTRUCTIONS {
                return Err(limit("has too many instructions"));
            }
            let id = self.instructions;
            self.instructions += 1;
            nodes.push(Node { id, instruction });
        }
        self.expect(b"}")?;
        Ok(nodes)
    }
}

fn parse_number(token: &str) -> Result<Value, OfdError> {
    let unsigned = token.strip_prefix(['+', '-']).unwrap_or(token);
    let mut dots = 0;
    let mut digits = 0;
    for byte in unsigned.bytes() {
        match byte {
            b'0'..=b'9' => digits += 1,
            b'.' => dots += 1,
            _ => return Err(invalid("unknown operator or invalid number")),
        }
    }
    if digits == 0 || dots > 1 {
        return Err(invalid("invalid number"));
    }
    if dots == 0 {
        if let Ok(integer) = token.parse::<i32>() {
            return Ok(Value::Integer(integer));
        }
    }
    let real = token
        .parse::<f64>()
        .map_err(|_| invalid("invalid real number"))?;
    if !real.is_finite() {
        return Err(invalid("nonfinite number"));
    }
    Ok(Value::Real(real))
}

fn tokens(bytes: &[u8]) -> Result<Vec<&[u8]>, OfdError> {
    let mut output = Vec::new();
    let mut at = 0;
    let whitespace = |byte: u8| matches!(byte, 0 | b'\t' | b'\n' | 12 | b'\r' | b' ');
    while at < bytes.len() {
        if whitespace(bytes[at]) {
            at += 1;
            continue;
        }
        if bytes[at] == b'%' {
            while at < bytes.len() && !matches!(bytes[at], b'\r' | b'\n') {
                at += 1;
            }
            continue;
        }
        let start = at;
        if matches!(bytes[at], b'{' | b'}') {
            at += 1;
        } else {
            while at < bytes.len()
                && !whitespace(bytes[at])
                && !matches!(bytes[at], b'{' | b'}' | b'%')
            {
                at += 1;
            }
        }
        output.push(&bytes[start..at]);
        if output.len() > MAX_INSTRUCTIONS * 4 {
            return Err(limit("has too many tokens"));
        }
    }
    Ok(output)
}

pub(crate) struct Calculator {
    nodes: Vec<Node>,
}

pub(crate) struct Calculation {
    pub values: Vec<f64>,
    /// Branch and integer-transition regions used to retain gradient jumps.
    pub regions: Vec<(usize, u64)>,
}

impl Calculator {
    pub fn parse(bytes: &[u8]) -> Result<Self, OfdError> {
        if bytes.len() > MAX_CODE_BYTES {
            return Err(limit("code exceeds 64 KiB"));
        }
        let mut parser = Parser {
            tokens: tokens(bytes)?,
            next: 0,
            instructions: 0,
        };
        let nodes = parser.block(0)?;
        if parser.next != parser.tokens.len() {
            return Err(invalid("trailing code after function"));
        }
        Ok(Self { nodes })
    }

    pub fn evaluate(&self, inputs: &[f64], outputs: usize) -> Result<Calculation, OfdError> {
        if inputs.len() > 32 || outputs == 0 || outputs > 32 {
            return Err(limit("has invalid input/output dimensions"));
        }
        let mut stack = Stack {
            values: Vec::new(),
            regions: Vec::new(),
        };
        for input in inputs {
            stack.push(Value::Real(*input))?;
        }
        execute(&self.nodes, &mut stack)?;
        let first = stack
            .values
            .len()
            .checked_sub(outputs)
            .ok_or_else(|| invalid("too few output values"))?;
        let values = stack.values[first..]
            .iter()
            .map(|value| value.number())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Calculation {
            values,
            regions: stack.regions,
        })
    }
}

struct Stack {
    values: Vec<Value>,
    regions: Vec<(usize, u64)>,
}

impl Stack {
    fn push(&mut self, value: Value) -> Result<(), OfdError> {
        if matches!(value, Value::Real(real) if !real.is_finite()) {
            return Err(invalid("undefined or nonfinite arithmetic result"));
        }
        if self.values.len() == MAX_STACK {
            return Err(limit("operand stack exceeds 256"));
        }
        self.values.push(value);
        Ok(())
    }

    fn pop(&mut self) -> Result<Value, OfdError> {
        self.values
            .pop()
            .ok_or_else(|| invalid("operand stack underflow"))
    }

    fn integer(&mut self) -> Result<i32, OfdError> {
        match self.pop()? {
            Value::Integer(value) => Ok(value),
            _ => Err(invalid("expected an integer")),
        }
    }

    fn boolean(&mut self) -> Result<bool, OfdError> {
        match self.pop()? {
            Value::Boolean(value) => Ok(value),
            _ => Err(invalid("expected a boolean")),
        }
    }

    fn number(&mut self) -> Result<f64, OfdError> {
        self.pop()?.number()
    }

    fn binary_number(
        &mut self,
        integer: fn(i32, i32) -> Option<i32>,
        real: fn(f64, f64) -> f64,
    ) -> Result<(), OfdError> {
        let right = self.pop()?;
        let left = self.pop()?;
        if let (Value::Integer(left), Value::Integer(right)) = (left, right) {
            if let Some(result) = integer(left, right) {
                return self.push(Value::Integer(result));
            }
        }
        self.push(Value::Real(real(left.number()?, right.number()?)))
    }
}

fn execute(nodes: &[Node], stack: &mut Stack) -> Result<(), OfdError> {
    for node in nodes {
        match &node.instruction {
            Instruction::Literal(value) => stack.push(*value)?,
            Instruction::Operator(operator) => apply(*operator, node.id, stack)?,
            Instruction::If(first) => {
                let condition = stack.boolean()?;
                stack.regions.push((node.id, u64::from(condition)));
                if condition {
                    execute(first, stack)?;
                }
            }
            Instruction::IfElse(first, second) => {
                let condition = stack.boolean()?;
                stack.regions.push((node.id, u64::from(condition)));
                execute(if condition { first } else { second }, stack)?;
            }
        }
    }
    Ok(())
}

fn apply(operator: Operator, id: usize, stack: &mut Stack) -> Result<(), OfdError> {
    use Operator::*;
    match operator {
        Add => stack.binary_number(i32::checked_add, |a, b| a + b)?,
        Sub => stack.binary_number(i32::checked_sub, |a, b| a - b)?,
        Mul => stack.binary_number(i32::checked_mul, |a, b| a * b)?,
        Div | Exp | Atan => {
            let right = stack.number()?;
            let left = stack.number()?;
            let value = match operator {
                Div => left / right,
                Exp => left.powf(right),
                _ => {
                    if left == 0.0 && right == 0.0 {
                        return Err(invalid("atan of two zero operands"));
                    }
                    let angle = left.atan2(right).to_degrees().rem_euclid(360.0);
                    stack.regions.push((id, u64::from(angle >= 180.0)));
                    angle
                }
            };
            stack.push(Value::Real(value))?;
        }
        Idiv | Mod | Bitshift => {
            let right = stack.integer()?;
            let left = stack.integer()?;
            let value = match operator {
                Idiv => left
                    .checked_div(right)
                    .ok_or_else(|| invalid("integer division result is undefined"))?,
                Mod if right == -1 => 0,
                Mod => left
                    .checked_rem(right)
                    .ok_or_else(|| invalid("integer remainder is undefined"))?,
                _ if right >= 32 => 0,
                _ if right <= -32 => {
                    if left < 0 {
                        -1
                    } else {
                        0
                    }
                }
                _ if right >= 0 => left.wrapping_shl(right as u32),
                _ => left >> right.unsigned_abs(),
            };
            stack.push(Value::Integer(value))?;
        }
        Abs | Neg => {
            let value = stack.pop()?;
            let result = match value {
                Value::Integer(integer) => {
                    let result = if matches!(operator, Abs) {
                        integer.checked_abs()
                    } else {
                        integer.checked_neg()
                    };
                    result
                        .map(Value::Integer)
                        .unwrap_or(Value::Real(2147483648.0))
                }
                value => Value::Real(if matches!(operator, Abs) {
                    value.number()?.abs()
                } else {
                    -value.number()?
                }),
            };
            stack.push(result)?;
        }
        Ceiling | Floor | Round | Truncate | Cvi | Cvr | Sqrt | Sin | Cos | Ln | Log => {
            let original = stack.pop()?;
            let input = original.number()?;
            let value = match operator {
                Ceiling => input.ceil(),
                Floor => input.floor(),
                Round => {
                    if input.fract() == 0.0 {
                        input
                    } else {
                        (input + 0.5).floor()
                    }
                }
                Truncate | Cvi => input.trunc(),
                Cvr => input,
                Sqrt => input.sqrt(),
                Sin => input.to_radians().sin(),
                Cos => input.to_radians().cos(),
                Ln => input.ln(),
                _ => input.log10(),
            };
            let result = if matches!(operator, Cvi) {
                if value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
                    return Err(invalid("cvi result is outside the integer range"));
                }
                Value::Integer(value as i32)
            } else if matches!(operator, Ceiling | Floor | Round | Truncate)
                && matches!(original, Value::Integer(_))
            {
                original
            } else {
                Value::Real(value)
            };
            if matches!(operator, Ceiling | Floor | Round | Truncate | Cvi) {
                stack.regions.push((id, value.to_bits()));
            }
            stack.push(result)?;
        }
        Eq | Ne | Gt | Ge | Lt | Le => {
            let right = stack.pop()?;
            let left = stack.pop()?;
            let result = if matches!(operator, Eq | Ne) {
                let equal = match (left, right) {
                    (Value::Boolean(left), Value::Boolean(right)) => left == right,
                    (Value::Boolean(_), _) | (_, Value::Boolean(_)) => false,
                    _ => left.number()? == right.number()?,
                };
                equal == matches!(operator, Eq)
            } else {
                let (left, right) = (left.number()?, right.number()?);
                match operator {
                    Gt => left > right,
                    Ge => left >= right,
                    Lt => left < right,
                    _ => left <= right,
                }
            };
            stack.push(Value::Boolean(result))?;
        }
        And | Or | Xor => {
            let right = stack.pop()?;
            let left = stack.pop()?;
            let result = match (left, right) {
                (Value::Boolean(a), Value::Boolean(b)) => Value::Boolean(match operator {
                    And => a & b,
                    Or => a | b,
                    _ => a ^ b,
                }),
                (Value::Integer(a), Value::Integer(b)) => Value::Integer(match operator {
                    And => a & b,
                    Or => a | b,
                    _ => a ^ b,
                }),
                _ => return Err(invalid("incompatible boolean/bitwise operands")),
            };
            stack.push(result)?;
        }
        Not => {
            let result = match stack.pop()? {
                Value::Boolean(value) => Value::Boolean(!value),
                Value::Integer(value) => Value::Integer(!value),
                _ => return Err(invalid("not requires an integer or boolean")),
            };
            stack.push(result)?;
        }
        Pop => {
            stack.pop()?;
        }
        Dup => {
            let value = stack
                .values
                .last()
                .copied()
                .ok_or_else(|| invalid("operand stack underflow"))?;
            stack.push(value)?;
        }
        Exch => {
            let size = stack.values.len();
            if size < 2 {
                return Err(invalid("operand stack underflow"));
            }
            stack.values.swap(size - 1, size - 2);
        }
        Copy | Index => {
            let count = stack.integer()?;
            if count < 0 {
                return Err(invalid("negative copy/index operand"));
            }
            let count = count as usize;
            let size = stack.values.len();
            if matches!(operator, Copy) {
                if count > size {
                    return Err(invalid("copy exceeds operand stack"));
                }
                if size + count > MAX_STACK {
                    return Err(limit("operand stack exceeds 256"));
                }
                stack.values.extend_from_within(size - count..);
            } else {
                let index = size
                    .checked_sub(count + 1)
                    .ok_or_else(|| invalid("index exceeds operand stack"))?;
                stack.push(stack.values[index])?;
            }
        }
        Roll => {
            let amount = stack.integer()?;
            let count = stack.integer()?;
            if count < 0 || count as usize > stack.values.len() {
                return Err(invalid("roll exceeds operand stack"));
            }
            if count > 0 {
                let first = stack.values.len() - count as usize;
                stack.values[first..].rotate_right(amount.rem_euclid(count) as usize);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(code: &str, inputs: &[f64], outputs: usize) -> Vec<f64> {
        Calculator::parse(code.as_bytes())
            .unwrap()
            .evaluate(inputs, outputs)
            .unwrap()
            .values
    }

    #[test]
    fn arithmetic_operators_preserve_postscript_numeric_semantics() {
        for (expression, expected) in [
            ("3 4 add", 7.0),
            ("3 4 sub", -1.0),
            ("3 4 mul", 12.0),
            ("3 2 div", 1.5),
            ("-7 2 idiv", -3.0),
            ("-7 2 mod", -1.0),
            ("-3 abs", 3.0),
            ("3 neg", -3.0),
            ("2 3 exp", 8.0),
            ("4 sqrt", 2.0),
            ("90 sin", 1.0),
            ("180 cos", -1.0),
            ("1 ln", 0.0),
            ("100 log", 2.0),
            ("-1 0 atan", 270.0),
            ("-2.5 round", -2.0),
            ("2.5 round", 3.0),
            ("2 round", 2.0),
            ("-2.8 truncate", -2.0),
            ("-2.8 cvi", -2.0),
            ("3 cvr", 3.0),
            ("-2.8 ceiling", -2.0),
            ("-2.8 floor", -3.0),
            ("2 ceiling", 2.0),
            ("2 floor", 2.0),
            ("2 truncate", 2.0),
            ("1.25 2 add", 3.25),
            ("1.5 2 mul", 3.0),
            ("1.5 2 sub", -0.5),
            ("-1.25 neg", 1.25),
            ("-1.25 abs", 1.25),
        ] {
            let code = format!("{{pop {expression}}}");
            let actual = values(&code, &[0.0], 1)[0];
            assert!(
                (actual - expected).abs() < 1e-12,
                "{expression}: {actual} != {expected}"
            );
        }
    }

    #[test]
    fn integer_overflow_promotes_arithmetic_and_unary_results_to_real() {
        for (expression, expected) in [
            ("2147483647 1 add", 2147483648.0),
            ("-2147483648 1 sub", -2147483649.0),
            ("2147483647 2 mul", 4294967294.0),
            ("-2147483648 neg", 2147483648.0),
            ("-2147483648 abs", 2147483648.0),
            ("2147483648 1 add", 2147483649.0),
            ("-2147483648 -1 mod", 0.0),
        ] {
            assert_eq!(
                values(&format!("{{pop {expression}}}"), &[0.0], 1),
                [expected],
                "{expression}"
            );
        }
    }

    #[test]
    fn logical_and_relational_operators_keep_booleans_distinct_from_numbers() {
        for (expression, expected) in [
            ("true false and", 0.0),
            ("true false or", 1.0),
            ("true true xor", 0.0),
            ("false not", 1.0),
            ("true true eq", 1.0),
            ("true false ne", 1.0),
            ("1 1.0 eq", 1.0),
            ("true 1 eq", 0.0),
            ("true 1 ne", 1.0),
            ("1 2 ne", 1.0),
            ("3 2 gt", 1.0),
            ("2 2 ge", 1.0),
            ("1 2 lt", 1.0),
            ("2 2 le", 1.0),
            ("3 2 le", 0.0),
        ] {
            let code = format!("{{pop {expression} {{1}} {{0}} ifelse}}");
            assert_eq!(values(&code, &[0.0], 1), [expected], "{expression}");
        }
    }

    #[test]
    fn bitwise_operators_use_bounded_signed_32_bit_operands() {
        for (expression, expected) in [
            ("6 3 and", 2.0),
            ("6 3 or", 7.0),
            ("6 3 xor", 5.0),
            ("0 not", -1.0),
            ("3 2 bitshift", 12.0),
            ("12 -2 bitshift", 3.0),
            ("-3 -1 bitshift", -2.0),
            ("1 31 bitshift", -2147483648.0),
            ("1 32 bitshift", 0.0),
            ("1 -32 bitshift", 0.0),
            ("-1 -32 bitshift", -1.0),
            ("1 -2147483648 bitshift", 0.0),
            ("1 0 bitshift", 1.0),
        ] {
            assert_eq!(
                values(&format!("{{pop {expression}}}"), &[0.0], 1),
                [expected],
                "{expression}"
            );
        }
    }

    #[test]
    fn stack_operators_preserve_order_and_handle_zero_counts() {
        for (code, expected) in [
            ("{pop 1 2 2 copy}", vec![1.0, 2.0, 1.0, 2.0]),
            ("{pop 1 2 0 copy}", vec![1.0, 2.0]),
            ("{pop 1 2 exch}", vec![2.0, 1.0]),
            ("{pop 1 2 3 2 index}", vec![1.0, 2.0, 3.0, 1.0]),
            ("{pop 1 2 3 3 1 roll}", vec![3.0, 1.0, 2.0]),
            ("{pop 1 2 3 3 -1 roll}", vec![2.0, 3.0, 1.0]),
            ("{pop 1 0 42 roll}", vec![1.0]),
            ("{dup mul}", vec![0.25]),
        ] {
            assert_eq!(values(code, &[0.5], expected.len()), expected, "{code}");
        }
        assert_eq!(values("{pop 1 2}", &[0.0], 1), [2.0]);
        assert_eq!(values("{pop true 1}", &[0.0], 1), [1.0]);
    }

    #[test]
    fn conditionals_short_circuit_and_record_region_changes() {
        let program = Calculator::parse(b"{dup .5 lt {dup mul} {1 exch sub} ifelse}").unwrap();
        let left = program.evaluate(&[0.25], 1).unwrap();
        let right = program.evaluate(&[0.75], 1).unwrap();
        assert_eq!(left.values, [0.0625]);
        assert_eq!(right.values, [0.25]);
        assert_ne!(left.regions, right.regions);
        assert_eq!(values("{true {2 mul} if}", &[0.25], 1), [0.5]);
        assert_eq!(values("{false {0 div} if}", &[0.25], 1), [0.25]);
        assert_eq!(
            values("{true {false {0 div} {2 mul} ifelse} if}", &[0.25], 1),
            [0.5]
        );
    }

    #[test]
    fn rounding_and_atan_record_discontinuous_regions() {
        for operation in ["floor", "ceiling", "round", "truncate", "cvi"] {
            let program = Calculator::parse(format!("{{{operation}}}").as_bytes()).unwrap();
            assert_ne!(
                program.evaluate(&[0.1], 1).unwrap().regions,
                program.evaluate(&[2.1], 1).unwrap().regions,
                "{operation}"
            );
        }
        let program = Calculator::parse(b"{1 atan}").unwrap();
        assert_ne!(
            program.evaluate(&[-0.1], 1).unwrap().regions,
            program.evaluate(&[0.1], 1).unwrap().regions
        );
    }

    #[test]
    fn accepts_pdf_decimal_syntax_comments_and_whitespace() {
        assert_eq!(values("{pop +.5 3. add -1 add}", &[0.0], 1), [2.5]);
        let program = Calculator::parse(b"\0{% non-UTF8 comment: \xff\r\n 2 mul}% end\n").unwrap();
        assert_eq!(program.evaluate(&[0.25], 1).unwrap().values, [0.5]);
    }

    #[test]
    fn rejects_code_outside_the_calculator_grammar() {
        for code in [
            "",
            "1",
            "{1",
            "{1}}",
            "{} {}",
            "{true {1} dup if}",
            "{true {1} {2} if}",
            "{if}",
            "{ifelse}",
            "{(text)}",
            "{[1 2]}",
            "{/name}",
            "{1e3}",
            "{16#ff}",
            "{NaN}",
            "{+}",
            "{1..2}",
            "{exec}",
        ] {
            assert!(Calculator::parse(code.as_bytes()).is_err(), "{code}");
        }
        assert!(Calculator::parse(b"{\xff}").is_err());
        let overflow = format!("{{{}}}", "9".repeat(400));
        assert!(Calculator::parse(overflow.as_bytes()).is_err());
    }

    #[test]
    fn rejects_stack_errors_type_errors_and_undefined_arithmetic() {
        for expression in [
            "pop",
            "dup",
            "exch",
            "1 copy",
            "-1 copy",
            "0 index",
            "-1 index",
            "1 0 roll",
            "-1 0 roll",
            "true 1 and",
            "1.0 not",
            "true 1 add",
            "true 1 sub",
            "true 1 mul",
            "true 1 div",
            "true neg",
            "true abs",
            "true sqrt",
            "false true lt",
            "1 {2} if",
            "true",
            "1 0 div",
            "-1 sqrt",
            "0 ln",
            "-1 log",
            "-1 .5 exp",
            "0 0 atan",
            "1 0 idiv",
            "1 0 mod",
            "-2147483648 -1 idiv",
            "2147483648 cvi",
            "9.9 1 idiv",
            "1.0 1 bitshift",
        ] {
            let code = format!("{{pop {expression}}}");
            assert!(
                Calculator::parse(code.as_bytes())
                    .unwrap()
                    .evaluate(&[0.0], 1)
                    .is_err(),
                "{expression}"
            );
        }
        let program = Calculator::parse(b"{}").unwrap();
        assert!(program.evaluate(&[], 1).is_err());
        assert!(program.evaluate(&[f64::NAN], 1).is_err());
        assert!(program.evaluate(&[f64::INFINITY], 1).is_err());
        assert!(program.evaluate(&[0.0; 33], 1).is_err());
        assert!(program.evaluate(&[0.0], 0).is_err());
        assert!(program.evaluate(&[0.0], 33).is_err());
    }

    #[test]
    fn enforces_code_instruction_nesting_and_operand_stack_limits() {
        assert!(matches!(
            Calculator::parse(&vec![b' '; MAX_CODE_BYTES + 1]),
            Err(OfdError::Limit(_))
        ));
        let instructions = format!("{{{}}}", "1 ".repeat(MAX_INSTRUCTIONS + 1));
        assert!(matches!(
            Calculator::parse(instructions.as_bytes()),
            Err(OfdError::Limit(_))
        ));
        let nested = format!(
            "{}{}",
            "true {".repeat(MAX_DEPTH + 2),
            "} if".repeat(MAX_DEPTH + 2)
        );
        assert!(matches!(
            Calculator::parse(format!("{{{nested}}}").as_bytes()),
            Err(OfdError::Limit(_))
        ));
        let stack = format!("{{{}}}", "1 ".repeat(MAX_STACK + 1));
        assert!(matches!(
            Calculator::parse(stack.as_bytes())
                .unwrap()
                .evaluate(&[], 1),
            Err(OfdError::Limit(_))
        ));
        let copy = format!("{{{}129 copy}}", "1 ".repeat(129));
        assert!(matches!(
            Calculator::parse(copy.as_bytes()).unwrap().evaluate(&[], 1),
            Err(OfdError::Limit(_))
        ));
    }
}
