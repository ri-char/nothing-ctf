use std::{collections::HashMap, fs, fmt::Display, io::Write};
use std::process::Command;

use gimli::UnwindSection;
use object::{Object, ObjectSection};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arg = std::env::args();
    if arg.len() != 2 {
        panic!("Argument Error!")
    }
    let bin_data = fs::read(arg.nth(1).unwrap())?;
    let obj_file = object::File::parse(&*bin_data)?;
    let data = obj_file.section_by_name(".eh_frame").unwrap();
    let eh_frame = gimli::read::EhFrame::new(data.data()?, gimli::LittleEndian);
    let bases = gimli::BaseAddresses::default().set_eh_frame(data.address());
    let mut entries = eh_frame.entries(&bases);

    let mut file = fs::OpenOptions::new().append(false).truncate(true).write(true).create(true).open("./output.c")?;
    writeln!(file, "#include <stdint.h>")?;


    let mut cies = HashMap::new();
    while let Some(entry) = entries.next()? {
        if let gimli::CieOrFde::Fde(partial) = entry {
            let fde = partial.parse(|_, bases, o| {
                cies.entry(o)
                    .or_insert_with(|| eh_frame.cie_from_offset(bases, o))
                    .clone()
            })?;
            // 通过长度过滤出我们想要的
            if fde.entry_len() < 100 {
                continue;
            }
            let mut instructions = fde.instructions(&eh_frame, &bases);
            use gimli::CallFrameInstruction::*;
            loop {
                match instructions.next() {
                    Err(e) => {
                        println!("Failed to decode CFI instruction: {}", e);
                        break;
                    }
                    Ok(Some(ValExpression {
                                register,
                                expression,
                            })) => {
                        println!(
                            "DW_CFA_val_expression ({}, ...)",
                            gimli::X86_64::register_name(register).unwrap_or("{unknown}")
                        );
                        display_val_expression(register, expression, &mut file)?;
                    }
                    Ok(None) => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
    file.flush()?;

    Command::new("gcc")
        .arg("-O3")
        .arg("./output.c")
        .arg("-c")
        .spawn()?;

    Ok(())
}

#[derive(Clone, Copy)]
struct Val {
    id: u64,
}

impl Val {
    fn new(id: u64) -> Self {
        Val { id }
    }
}

struct ValGenerator {
    id: u64,
}

impl ValGenerator {
    fn new() -> Self {
        Self { id: 0 }
    }
    fn next(&mut self) -> Val {
        self.id += 1;
        Val::new(self.id - 1)
    }
}

impl Display for Val {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "v{}", self.id)
    }
}

fn display_val_expression<R>(target_reg: gimli::Register, exp: gimli::Expression<R>, w: &mut dyn Write) -> Result<(), Box<dyn std::error::Error>>
    where
        R: gimli::Reader,
{
    let mut val_generator = ValGenerator::new();
    let mut ops = exp.operations(gimli::Encoding { address_size: 8, format: gimli::Format::Dwarf64, version: 5 });
    let mut stack: Vec<Val> = Vec::new();
    writeln!(w, "uint64_t cal_{}(uint64_t r12, uint64_t r13, uint64_t r14, uint64_t r15){{", gimli::X86_64::register_name(target_reg).unwrap())?;
    writeln!(w, "    uint64_t rax=0,rbx=0;")?;
    loop {
        if let Ok(Some(op)) = ops.next() {
            match op {
                gimli::Operation::Drop => {
                    stack.pop();
                }
                gimli::Operation::Pick { index } => {
                    let val1 = stack.get(stack.len() - 1 - index as usize).unwrap();
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}={};", new_val, val1)?;
                    stack.push(new_val);
                }
                gimli::Operation::Swap => {
                    let val1 = stack.pop().unwrap();
                    let val2 = stack.pop().unwrap();
                    stack.push(val1);
                    stack.push(val2);
                }
                gimli::Operation::Rot => {
                    let val1 = stack.pop().unwrap();
                    let val2 = stack.pop().unwrap();
                    let val3 = stack.pop().unwrap();
                    stack.push(val1);
                    stack.push(val3);
                    stack.push(val2);
                }
                gimli::Operation::And => {
                    let val1 = stack.pop().unwrap();
                    let val2 = stack.pop().unwrap();
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}={}&{};", new_val, val2, val1)?;
                    stack.push(new_val);
                }
                gimli::Operation::Minus => {
                    let val1 = stack.pop().unwrap();
                    let val2 = stack.pop().unwrap();
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}={}-{};", new_val, val2, val1)?;
                    stack.push(new_val);
                }
                gimli::Operation::Neg => {
                    let val = stack.get(stack.len() - 1).unwrap();
                    writeln!(w, "    {}=-{};", val, val)?;
                }
                gimli::Operation::Not => {
                    let val = stack.get(stack.len() - 1).unwrap();
                    writeln!(w, "    {}=~{};", val, val)?;
                }
                gimli::Operation::Or => {
                    let val1 = stack.pop().unwrap();
                    let val2 = stack.pop().unwrap();
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}={}|{};", new_val, val2, val1)?;
                    stack.push(new_val);
                }
                gimli::Operation::Plus => {
                    let val1 = stack.pop().unwrap();
                    let val2 = stack.pop().unwrap();
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}={}+{};", new_val, val2, val1)?;
                    stack.push(new_val);
                }
                gimli::Operation::PlusConstant { value } => {
                    let val = stack.get(stack.len() - 1).unwrap();
                    writeln!(w, "    {}+={}ull;", val, value)?;
                }
                gimli::Operation::Shl => {
                    let val1 = stack.pop().unwrap();
                    let val2 = stack.pop().unwrap();
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}={}<<{};", new_val, val2, val1)?;
                    stack.push(new_val);
                }
                gimli::Operation::Shr => {
                    let val1 = stack.pop().unwrap();
                    let val2 = stack.pop().unwrap();
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}={}>>{};", new_val, val2, val1)?;
                    stack.push(new_val);
                }
                gimli::Operation::Shra => {
                    let val1 = stack.pop().unwrap();
                    let val2 = stack.pop().unwrap();
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}=(uint64_t)((int64_t){}>>(int64_t){});", new_val, val2, val1)?;
                    stack.push(new_val);
                }
                gimli::Operation::Xor => {
                    let val1 = stack.pop().unwrap();
                    let val2 = stack.pop().unwrap();
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}={}^{};", new_val, val2, val1)?;
                    stack.push(new_val);
                }
                gimli::Operation::Eq => {
                    let val1 = stack.pop().unwrap();
                    let val2 = stack.pop().unwrap();
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}= {}=={}?1:0;", new_val, val2, val1)?;
                    stack.push(new_val);
                }
                gimli::Operation::Ge => {
                    let val1 = stack.pop().unwrap();
                    let val2 = stack.pop().unwrap();
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}={}>={}?1:0;", new_val, val2, val1)?;
                    stack.push(new_val);
                }
                gimli::Operation::Gt => {
                    let val1 = stack.pop().unwrap();
                    let val2 = stack.pop().unwrap();
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}={}>{}?1:0;", new_val, val2, val1)?;
                    stack.push(new_val);
                }
                gimli::Operation::Le => {
                    let val1 = stack.pop().unwrap();
                    let val2 = stack.pop().unwrap();
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}={}<={}?1:0;", new_val, val2, val1)?;
                    stack.push(new_val);
                }
                gimli::Operation::Lt => {
                    let val1 = stack.pop().unwrap();
                    let val2 = stack.pop().unwrap();
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}={}<{}?1:0;", new_val, val2, val1)?;
                    stack.push(new_val);
                }
                gimli::Operation::Ne => {
                    let val1 = stack.pop().unwrap();
                    let val2 = stack.pop().unwrap();
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}={}!={}?1:0;", new_val, val2, val1)?;
                    stack.push(new_val);
                }
                gimli::Operation::UnsignedConstant { value } => {
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}={}ull;", new_val, value)?;
                    stack.push(new_val);
                }
                gimli::Operation::SignedConstant { value } => {
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}=(uint64_t){}ll;", new_val, value)?;
                    stack.push(new_val);
                }
                gimli::Operation::Register { register } => {
                    let new_val = val_generator.next();
                    writeln!(w, "    uint64_t {}={};", new_val, gimli::X86_64::register_name(register).unwrap_or("{error}"))?;
                    stack.push(new_val);
                }
                _ => todo!("{:?}", op)
            }
        } else {
            break;
        }
    }
    assert_eq!(stack.len(), 1);
    writeln!(w, "    return {};", stack.pop().unwrap())?;
    writeln!(w, "}}\n")?;
    Ok(())
}
