use std::mem::swap;

use gimli::write::{Expression,CallFrameInstruction};
use rand::{Rng, prelude::ThreadRng};

fn exp_constu(exp:&mut Expression, rng:&mut ThreadRng,num:u64,depth:u64){
    if depth==0 {
        exp.op_constu(num);
        return;
    }
    let ty:usize=rng.gen_range(0..9);
    match ty{
        0=>{
            let xor_num:u64=rng.gen();
            exp_constu(exp, rng, xor_num, depth-1);
            exp_constu(exp, rng, xor_num^num, depth-1);
            exp.op(gimli::DW_OP_xor);
        }
        1=>{
            let add_num:u64=rng.gen();
            exp_constu(exp, rng, add_num.wrapping_add(num), depth-1);
            exp_constu(exp, rng, add_num, depth-1);
            exp.op(gimli::DW_OP_minus);
        }
        2=>{
            let add_num:u64=rng.gen();
            exp_constu(exp, rng, num.wrapping_sub(add_num), depth-1);
            exp_constu(exp, rng, add_num, depth-1);
            exp.op(gimli::DW_OP_plus);
        }
        3=>{
            let num1:u64=rng.gen::<u64>()&num;
            let mut num2:u64=rng.gen::<u64>()&num;
            num2|=num&!(num1|num2);
            assert_eq!(num1|num2,num);
            exp_constu(exp, rng, num1, depth-1);
            exp_constu(exp, rng, num2, depth-1);
            exp.op(gimli::DW_OP_or);
        }
        4=>{
            let num1:u64=rng.gen::<u64>()&!num;
            let mut num2:u64=rng.gen::<u64>()&!num;
            num2|=!num&!(num1|num2);
            assert_eq!(!num1&!num2,num);
            exp_constu(exp, rng, !num1, depth-1);
            exp_constu(exp, rng, !num2, depth-1);
            exp.op(gimli::DW_OP_and);
        }
        5=>{
            exp_constu(exp, rng, !num, depth-1);
            exp.op(gimli::DW_OP_not);
        }
        6=>{
            exp.op(gimli::DW_OP_dup);
            exp_constu(exp, rng, num, depth-1);
            exp.op(gimli::DW_OP_swap);
            exp.op(gimli::DW_OP_drop);
        }
        7=>{
            let rand_num:u64=rng.gen();
            exp_constu(exp, rng, rand_num, depth-1);
            exp_constu(exp, rng, num, depth-1);
            exp.op(gimli::DW_OP_swap);
            exp.op(gimli::DW_OP_drop);
        }
        8=>{
            let rand_num1:u64=rng.gen();
            let rand_num2:u64=rng.gen();
            exp_constu(exp, rng, rand_num1, depth-1);
            exp_constu(exp, rng, rand_num2, depth-1);
            exp_constu(exp, rng, num, depth-1);
            exp.op(gimli::DW_OP_rot);
            exp.op(gimli::DW_OP_drop);
            exp.op(gimli::DW_OP_drop);
        }
        _=>{}
    }
    
}

const DEFAULT_DEPTH:u64=9;

#[derive(Debug)]
struct Arg{
    flag_a:u64,
    flag_b:u64,
    det:u64,
    round:u64,
    xor_num_a:u64,
    xor_num_b:u64,
    hash_num:u64,
}

impl rand::prelude::Distribution<Arg> for rand::distributions::Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Arg {
        let flag_a:u64=rng.gen();
        let flag_b:u64=rng.gen();
        let det:u64=rng.gen();
        let round:u64=rng.gen_range(16..=32);
        let xor_num_a:u64=rng.gen();
        let xor_num_b:u64=rng.gen();
        let hash_num:u64=rng.gen();
        Arg { flag_a, flag_b, det, round, xor_num_a, xor_num_b, hash_num }
    }
}

impl Arg {
    fn enc(&self)->(u64,u64){
        let mut det=0;
        let mut r14=self.flag_a;
        let mut r15=self.flag_b;

        r14^=self.xor_num_a;
        r15^=self.xor_num_b;
        while det!=self.round.wrapping_mul(self.det) {
            det=det.wrapping_add(self.det);
            r14^=(det.wrapping_add(self.hash_num))^(det>>30)^(r15<<24);
            swap(&mut r14, &mut r15);
        }
        r14^=self.xor_num_a;
        r15^=self.xor_num_b;
        (r14,r15)
    }

    fn generate_code(&self,rng:&mut ThreadRng)->Vec<u8>{
        let mut w = gimli::write::EndianVec::new(gimli::LittleEndian);
        let (ans_a,ans_b)=self.enc();
        // r13 += det                                           0
        // r14 ^= f(r13,r15)                                    1
        // f(r13,r15) = (r13+hash_num)^(r13>>30)^(r15<<24)
        // r14,r15 = r15,r14                                    2
        // r14 ^= xor_num_a  r15^= xor_num_b                    3
        // r12 = r12 >> 8                                       4
        // r12 = 0                                              5
        // r12 = r13 == (det*round) ? (r12 >> 8) : 2            6
        // r13 = (r14!=ans_a | r15!=ans_b)                      7

        // r12
        {
            // r12 = ((r12&0xff)==0)-1)
            let mut exp=Expression::new();
            exp.op_reg(gimli::X86_64::R12);
            exp_constu(&mut exp, rng, 0xff, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_and);
            // <=3
            exp.op(gimli::DW_OP_dup);
            exp_constu(&mut exp, rng, 3, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_le);
            exp.op(gimli::DW_OP_swap);

            // <=3 | ==4
            exp.op(gimli::DW_OP_dup);
            // exp_constu(&mut exp, rng, 4, DEFAULT_DEPTH);
            {
                exp_constu(&mut exp, rng, 1, DEFAULT_DEPTH);
                exp.op(gimli::DW_OP_dup);
                exp.op(gimli::DW_OP_dup);
                exp.op(gimli::DW_OP_shl);
                exp.op(gimli::DW_OP_shl);
            }
            exp.op(gimli::DW_OP_ne);
            exp_constu(&mut exp, rng, 1, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_minus);
            exp.op_reg(gimli::X86_64::R12);
            // exp_constu(&mut exp, rng, 8, DEFAULT_DEPTH);
            {
                exp_constu(&mut exp, rng, 1, DEFAULT_DEPTH);
                exp.op(gimli::DW_OP_dup);
                exp.op(gimli::DW_OP_dup);
                exp.op(gimli::DW_OP_dup);
                exp.op(gimli::DW_OP_shl);
                exp.op(gimli::DW_OP_or);
                exp.op(gimli::DW_OP_shl);
            }
            exp.op(gimli::DW_OP_shr);
            exp_constu(&mut exp, rng, 64-8, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_shl);
            exp_constu(&mut exp, rng, 64-8, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_shra);
            exp.op(gimli::DW_OP_and);
            exp.op(gimli::DW_OP_swap);

            // <=3 | ==4 | ==6
            exp_constu(&mut exp, rng, 6, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_ne);
            exp_constu(&mut exp, rng, 1, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_minus);         // eq 6
            exp.op_reg(gimli::X86_64::R13);
            exp_constu(&mut exp, rng, self.round.wrapping_mul(self.det), DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_ne);
            exp_constu(&mut exp, rng, 1, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_minus);         // eq 6 | r13 == (det*round)
            exp.op(gimli::DW_OP_dup);
            exp.op(gimli::DW_OP_not);           // eq 6 | r13 == (det*round) | r13 != (det*round)
            exp_constu(&mut exp, rng, 2, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_and);
            exp.op(gimli::DW_OP_swap);          // eq 6 | r13 != (det*round)?2:0 | r13 == (det*round)
            exp.op_reg(gimli::X86_64::R12);
            exp_constu(&mut exp, rng, 8, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_shr);
            exp_constu(&mut exp, rng, 64-8, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_shl);
            exp_constu(&mut exp, rng, 64-8, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_shra);
            exp.op(gimli::DW_OP_and);           // eq 6 | r13 != (det*round)?2:0 | r13 == (det*round)?next:0
            exp.op(gimli::DW_OP_or);
            exp.op(gimli::DW_OP_and);

            exp.op(gimli::DW_OP_or);
            exp.op(gimli::DW_OP_or);
            let cf=CallFrameInstruction::ValExpression(gimli::X86_64::R12, exp);
            cf.simple_write(&mut w,gimli::Encoding { address_size: 8, format: gimli::Format::Dwarf64, version: 5 }).unwrap();
        }

        // r15
        {
            let mut exp=Expression::new();
            exp.op_reg(gimli::X86_64::R12);
            exp_constu(&mut exp, rng, 0xff, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_and);
            exp.op(gimli::DW_OP_dup);
            exp_constu(&mut exp, rng, 2, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_ne);
            exp_constu(&mut exp, rng, 1, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_minus);
            exp.op(gimli::DW_OP_swap);
            exp_constu(&mut exp, rng, 3, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_ne);
            exp_constu(&mut exp, rng, 1, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_minus);
            exp.op(gimli::DW_OP_dup);
            exp.op_pick(2);
            exp.op(gimli::DW_OP_or);
            exp.op(gimli::DW_OP_not);

            // is 2 | is 3 | else $
            exp.op_reg(gimli::X86_64::R15);
            exp.op(gimli::DW_OP_and);
            exp.op(gimli::DW_OP_rot);

            // else | is 2 | is 3 $
            exp.op_reg(gimli::X86_64::R15);
            exp_constu(&mut exp, rng, self.xor_num_b, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_xor);
            exp.op(gimli::DW_OP_and);
            exp.op(gimli::DW_OP_rot);

            // is 3 | else | is 2 $
            exp.op_reg(gimli::X86_64::R14);
            exp.op(gimli::DW_OP_and);

            exp.op(gimli::DW_OP_or);
            exp.op(gimli::DW_OP_or);

            let cf=CallFrameInstruction::ValExpression(gimli::X86_64::R15, exp);

            cf.simple_write(&mut w,gimli::Encoding { address_size: 8, format: gimli::Format::Dwarf64, version: 5 }).unwrap();
        }

        // r13
        {
            let mut exp=Expression::new();
            exp.op_reg(gimli::X86_64::R12);
            exp_constu(&mut exp, rng, 0xff, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_and);
            
            // ==0
            exp.op(gimli::DW_OP_dup);
            exp_constu(&mut exp, rng, 0, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_ne);
            exp_constu(&mut exp, rng, 1, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_minus);
            exp_constu(&mut exp, rng, self.det, DEFAULT_DEPTH);
            exp.op_reg(gimli::X86_64::R13);
            exp.op(gimli::DW_OP_plus);
            exp.op(gimli::DW_OP_and);
            exp.op(gimli::DW_OP_swap);

            // ==0 | ==7
            exp.op(gimli::DW_OP_dup);
            // exp_constu(&mut exp, rng, 7, DEFAULT_DEPTH);
            {
                exp_constu(&mut exp, rng, 0, DEFAULT_DEPTH);
                exp.op(gimli::DW_OP_not);
                exp_constu(&mut exp, rng, 64-3, DEFAULT_DEPTH);
                exp.op(gimli::DW_OP_shr);
            }
            exp.op(gimli::DW_OP_eq);
            {
                exp.op_reg(gimli::X86_64::R14);
                exp_constu(&mut exp, rng, ans_a, DEFAULT_DEPTH);
                exp.op(gimli::DW_OP_ne);
                exp.op_reg(gimli::X86_64::R15);
                exp_constu(&mut exp, rng, ans_b, DEFAULT_DEPTH);
                exp.op(gimli::DW_OP_ne);
                exp.op(gimli::DW_OP_or);
            }
            exp.op(gimli::DW_OP_and);
            exp.op(gimli::DW_OP_swap);

            // ==0 | ==7 | else
            exp.op(gimli::DW_OP_dup);
            // exp_constu(&mut exp, rng, 0, DEFAULT_DEPTH);
            {
                exp.op_reg(gimli::X86_64::RAX);
                exp.op_reg(gimli::X86_64::RBX);
                exp.op(gimli::DW_OP_dup);
                exp.op(gimli::DW_OP_not);
                exp.op(gimli::DW_OP_and);
                exp.op(gimli::DW_OP_and);
            }
            exp.op(gimli::DW_OP_eq);
            exp.op(gimli::DW_OP_swap);
            exp_constu(&mut exp, rng, 7, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_eq);
            exp.op(gimli::DW_OP_or);
            exp_constu(&mut exp, rng, 1, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_minus);
            exp.op_reg(gimli::X86_64::R13);


            exp.op(gimli::DW_OP_and);


            exp.op(gimli::DW_OP_or);
            exp.op(gimli::DW_OP_or);

            let cf=CallFrameInstruction::ValExpression(gimli::X86_64::R13, exp);
            cf.simple_write(&mut w,gimli::Encoding { address_size: 8, format: gimli::Format::Dwarf64, version: 5 }).unwrap();
        }

        // r14
        {
            let mut exp=Expression::new();
            exp.op_reg(gimli::X86_64::R12);
            exp_constu(&mut exp, rng, 0xff, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_and);

            // else $
            exp.op(gimli::DW_OP_dup);
            exp.op(gimli::DW_OP_dup);
            // exp_constu(&mut exp, rng, 0, DEFAULT_DEPTH);
            {
                exp.op_reg(gimli::X86_64::R15);
                exp.op(gimli::DW_OP_dup);
                exp.op(gimli::DW_OP_not);
                exp.op(gimli::DW_OP_and);
            }
            exp.op(gimli::DW_OP_gt);
            exp.op(gimli::DW_OP_swap);
            // exp_constu(&mut exp, rng, 4, DEFAULT_DEPTH);
            {
                exp.op_reg(gimli::X86_64::RAX);
                exp.op(gimli::DW_OP_dup);
                exp.op(gimli::DW_OP_not);
                exp.op(gimli::DW_OP_and);
                exp.op_plus_uconst(1);
                exp.op(gimli::DW_OP_dup);
                exp.op(gimli::DW_OP_dup);
                exp.op(gimli::DW_OP_shl);
                exp.op(gimli::DW_OP_shl);
            }
            exp.op(gimli::DW_OP_lt);
            exp.op(gimli::DW_OP_and);
            exp_constu(&mut exp, rng, 1, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_minus);
            exp.op_reg(gimli::X86_64::R14);
            exp.op(gimli::DW_OP_and);
            exp.op(gimli::DW_OP_swap);

            // else | ==2 $
            exp.op(gimli::DW_OP_dup);
            exp_constu(&mut exp, rng, 2, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_ne);
            exp_constu(&mut exp, rng, 1, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_minus);
            exp.op_reg(gimli::X86_64::R15);
            exp.op(gimli::DW_OP_and);
            exp.op(gimli::DW_OP_swap);

            // else | ==2 | ==3 $
            exp.op(gimli::DW_OP_dup);
            exp_constu(&mut exp, rng, 3, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_ne);
            exp_constu(&mut exp, rng, 1, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_minus);
            exp.op_reg(gimli::X86_64::R14);
            exp_constu(&mut exp, rng, self.xor_num_a, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_xor);
            exp.op(gimli::DW_OP_and);
            exp.op(gimli::DW_OP_swap);

            // else | ==2 | ==3 | ==1 $
            exp_constu(&mut exp, rng, 1, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_ne);
            exp_constu(&mut exp, rng, 1, DEFAULT_DEPTH);
            exp.op(gimli::DW_OP_minus);

            exp.op_reg(gimli::X86_64::R14);
            // f(r13,r15) = (r13+hash_num)^(r13>>30)^(r15<<24)
            {
                exp.op_reg(gimli::X86_64::R13);
                exp.op(gimli::DW_OP_dup);
                exp.op_plus_uconst(self.hash_num);
                exp.op(gimli::DW_OP_swap);
                exp_constu(&mut exp, rng, 30, DEFAULT_DEPTH);
                exp.op(gimli::DW_OP_shr);

                exp.op_reg(gimli::X86_64::R15);
                exp_constu(&mut exp, rng, 24, DEFAULT_DEPTH);
                exp.op(gimli::DW_OP_shl);

                exp.op(gimli::DW_OP_xor);
                exp.op(gimli::DW_OP_xor);
            }
            exp.op(gimli::DW_OP_xor);
            exp.op(gimli::DW_OP_and);

            exp.op(gimli::DW_OP_or);
            exp.op(gimli::DW_OP_or);
            exp.op(gimli::DW_OP_or);

            let cf=CallFrameInstruction::ValExpression(gimli::X86_64::R14, exp);
            cf.simple_write(&mut w,gimli::Encoding { address_size: 8, format: gimli::Format::Dwarf64, version: 5 }).unwrap();
        }

        w.slice().to_vec()
    }
}

fn main() {
    let mut rng=rand::thread_rng();
    let args:Arg=rng.gen();

    let codes=args.generate_code(&mut rng);
    
    eprintln!("flag{{{:016x}{:016x}}}",args.flag_a,args.flag_b);
    // eprintln!("len: {}",codes.len());
    // eprintln!("{:#?}",args);
    
    let bytecode=codes.iter().map(|x|x.to_string()).collect::<Vec<String>>().join(",");
    println!("asm(\".cfi_escape {}\");",bytecode);
}
