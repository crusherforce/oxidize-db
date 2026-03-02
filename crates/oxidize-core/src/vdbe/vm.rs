use super::opcodes::Opcode;
use crate::error::OxidizeError;
use crate::schema::Value;
use crate::Result;

const NUM_REGISTERS: usize = 256;

/// A row of values produced by the VM as output.
pub type Row = Vec<Value>;

/// Execution context for a single VDBE program.
pub struct VirtualMachine {
    /// Register file.
    registers: Vec<Value>,
    /// Program counter.
    pc: usize,
    /// Collected output rows.
    output: Vec<Row>,
}

impl VirtualMachine {
    pub fn new() -> Self {
        Self {
            registers: vec![Value::Null; NUM_REGISTERS],
            pc: 0,
            output: Vec::new(),
        }
    }

    fn reg(&self, idx: usize) -> &Value {
        &self.registers[idx]
    }

    fn reg_mut(&mut self, idx: usize) -> &mut Value {
        &mut self.registers[idx]
    }

    /// Execute a VDBE program and return any result rows.
    ///
    /// This is a stub implementation that handles only simple scalar
    /// opcodes (Integer, String, Null, Yield, Halt). Full table-scan
    /// opcodes require integrating with BTree and Pager.
    pub fn execute(&mut self, program: &[Opcode]) -> Result<Vec<Row>> {
        self.pc = 0;
        self.output.clear();

        loop {
            if self.pc >= program.len() {
                break;
            }
            let op = &program[self.pc];
            match op {
                Opcode::Init { addr } => {
                    self.pc = *addr;
                    continue;
                }
                Opcode::Halt => break,
                Opcode::Goto { addr } => {
                    self.pc = *addr;
                    continue;
                }
                Opcode::Integer { value, reg } => {
                    *self.reg_mut(*reg) = Value::Integer(*value);
                }
                Opcode::String { value, reg } => {
                    *self.reg_mut(*reg) = Value::Text(value.clone());
                }
                Opcode::Null { reg } => {
                    *self.reg_mut(*reg) = Value::Null;
                }
                Opcode::Copy { src, dest } => {
                    let val = self.reg(*src).clone();
                    *self.reg_mut(*dest) = val;
                }
                Opcode::Add { left, right, dest } => {
                    let result = match (self.reg(*left), self.reg(*right)) {
                        (Value::Integer(a), Value::Integer(b)) => Value::Integer(a + b),
                        (Value::Real(a), Value::Real(b)) => Value::Real(a + b),
                        (Value::Integer(a), Value::Real(b)) => Value::Real(*a as f64 + b),
                        (Value::Real(a), Value::Integer(b)) => Value::Real(a + *b as f64),
                        _ => {
                            return Err(OxidizeError::Unsupported(
                                "ADD requires numeric operands".into(),
                            ))
                        }
                    };
                    *self.reg_mut(*dest) = result;
                }
                Opcode::Yield { reg } => {
                    let val = self.reg(*reg).clone();
                    self.output.push(vec![val]);
                }
                Opcode::ResultRow { start, count } => {
                    let row: Vec<Value> =
                        (0..*count).map(|i| self.reg(*start + i).clone()).collect();
                    self.output.push(row);
                }
                Opcode::Eq { left, right, addr } => {
                    if self.reg(*left) == self.reg(*right) {
                        self.pc = *addr;
                        continue;
                    }
                }
                // Cursor-based opcodes require BTree integration — stub for now.
                Opcode::OpenRead { .. }
                | Opcode::OpenWrite { .. }
                | Opcode::Rewind { .. }
                | Opcode::Column { .. }
                | Opcode::Next { .. }
                | Opcode::Insert { .. }
                | Opcode::Delete { .. } => {
                    return Err(OxidizeError::Unsupported(
                        "cursor opcodes not yet implemented".into(),
                    ));
                }
            }
            self.pc += 1;
        }

        Ok(std::mem::take(&mut self.output))
    }
}

impl Default for VirtualMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integer_yield() {
        let program = vec![
            Opcode::Integer { value: 42, reg: 0 },
            Opcode::Yield { reg: 0 },
            Opcode::Halt,
        ];
        let mut vm = VirtualMachine::new();
        let rows = vm.execute(&program).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], Value::Integer(42));
    }

    #[test]
    fn test_add() {
        let program = vec![
            Opcode::Integer { value: 10, reg: 0 },
            Opcode::Integer { value: 32, reg: 1 },
            Opcode::Add {
                left: 0,
                right: 1,
                dest: 2,
            },
            Opcode::Yield { reg: 2 },
            Opcode::Halt,
        ];
        let mut vm = VirtualMachine::new();
        let rows = vm.execute(&program).unwrap();
        assert_eq!(rows[0][0], Value::Integer(42));
    }

    #[test]
    fn test_result_row() {
        let program = vec![
            Opcode::Integer { value: 1, reg: 0 },
            Opcode::String {
                value: "hello".into(),
                reg: 1,
            },
            Opcode::ResultRow { start: 0, count: 2 },
            Opcode::Halt,
        ];
        let mut vm = VirtualMachine::new();
        let rows = vm.execute(&program).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], Value::Integer(1));
        assert_eq!(rows[0][1], Value::Text("hello".into()));
    }
}
