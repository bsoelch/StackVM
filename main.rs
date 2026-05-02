use std::fs::File;
use std::io::Read;
use std::io;

fn bytes_as_u32(bytes: &[u8]) -> Option<&[i32]> {
  if bytes.len() % 4 != 0 {
    return None;
  }
  unsafe {
    let ptr = bytes.as_ptr() as *const i32;
    let len = bytes.len() / 4;
    Some(std::slice::from_raw_parts(ptr, len))
  }
}
fn bytes_as_u64(bytes: &[u8]) -> Option<&[i64]> {
  if bytes.len() % 8 != 0 {
    return None;
  }
  unsafe {
    let ptr = bytes.as_ptr() as *const i64;
    let len = bytes.len() / 8;
    Some(std::slice::from_raw_parts(ptr, len))
  }
}
fn u64_as_bytes_mut(ints: &mut [u64]) -> &mut[u8] {
  unsafe {
    let ptr = ints.as_ptr() as *mut u8;
    let len = ints.len() * 8;
    std::slice::from_raw_parts_mut(ptr, len)
  }
}
fn u64_as_bytes(ints: &[u64]) -> &[u8] {
  unsafe {
    let ptr = ints.as_ptr() as *const u8;
    let len = ints.len() * 8;
    std::slice::from_raw_parts(ptr, len)
  }
}
fn u32_as_bytes_mut(ints: &mut [u32]) -> &mut[u8] {
  unsafe {
    let ptr = ints.as_ptr() as *mut u8;
    let len = ints.len() * 4;
    std::slice::from_raw_parts_mut(ptr, len)
  }
}

// use high two bits as tag
const PTR_TYPE_MASK: u64 = 0xC000_0000_0000_0000;
const PTR_VALUE_MASK: u64 = 0x3fff_ffff_ffff_ffff;
const PTR_LOCAL: u64  = 0x0000_0000_0000_0000;
const PTR_RODATA: u64 = 0x4000_0000_0000_0000;
const PTR_RWDATA: u64 = 0x8000_0000_0000_0000;
const PTR_HEAP: u64   = 0xC000_0000_0000_0000;
fn local_addr_to_ptr(local_addr: usize) -> u64 {
  if local_addr as u64 > PTR_VALUE_MASK {panic!("address outside allowed range");}
  return local_addr as u64 | PTR_LOCAL;
}

struct Program{
  code: Box<[u32]>,
  rodata: Box<[u64]>,
  rwdata: Box<[u64]>,
}
fn stack_get(stack: &[u64],index: usize) -> u64 {
  if index == 0 || stack.len() < index {
    panic!("stack index out of range {}",index);
  }
  return stack[stack.len()-index];
}
fn stack_set(new_val: u64,stack: &mut Vec<u64>,index: usize) {
  if index == 0 {
    stack.push(new_val);
    return
  }
  if stack.len() < index {
    panic!("stack index out of range {}",index);
  }
  let index = stack.len() - index;
  stack[index] = new_val;
}
fn buffer_get(_memory: &[u8],_addr: usize,_dst: &mut[u8]) {
  panic!("unimplemented: buffer_get");
}
fn buffer_set(_memory: &mut [u8],_addr: usize,_src: &[u8]) {
  panic!("unimplemented: buffer_set");
}

const VAL_I8: u32 = 0;
const VAL_I16: u32 = 1;
const VAL_I32: u32 = 2;
const VAL_I64: u32 = 3;
const VAL_FLOAT: u32 = 4;
const VAL_F16: u32 = 5;
const VAL_F32: u32 = 6;
const VAL_F64: u32 = 7;
const CMP_EQ: u32 = 0;
const CMP_NE: u32 = 1;
// CMP_2,CMP_3
const CMP_LT: u32 = 4;
const CMP_LE: u32 = 5;
const CMP_ULT: u32 = 6;
const CMP_ULE: u32 = 7;

// TODO? support operations on f16
fn op_cmp(arg1: u64,arg2: u64, cmp_type: u32,val_type: u32) -> u64 {
  match (val_type,cmp_type) {
    (VAL_I8, CMP_EQ)  => {((arg1 as u8) == (arg2 as u8)) as u64 }
    (VAL_I16, CMP_EQ) => {((arg1 as u16) == (arg2 as u16)) as u64 }
    (VAL_I32, CMP_EQ) => {((arg1 as u32) == (arg2 as u32)) as u64 }
    (VAL_I64, CMP_EQ) => {(arg1 == arg2) as u64 }
    (VAL_F32, CMP_EQ) => {(f32::from_bits(arg1 as u32) == f32::from_bits(arg2 as u32)) as u64 }
    (VAL_F64, CMP_EQ) => {(f64::from_bits(arg1) == f64::from_bits(arg2)) as u64 }
    (VAL_I8, CMP_NE)  => {((arg1 as u8) != (arg2 as u8)) as u64 }
    (VAL_I16, CMP_NE) => {((arg1 as u16) != (arg2 as u16)) as u64 }
    (VAL_I32, CMP_NE) => {((arg1 as u32) != (arg2 as u32)) as u64 }
    (VAL_I64, CMP_NE) => {(arg1 != arg2) as u64 }
    (VAL_F32, CMP_NE) => {(f32::from_bits(arg1 as u32) != f32::from_bits(arg2 as u32)) as u64 }
    (VAL_F64, CMP_NE) => {(f64::from_bits(arg1) != f64::from_bits(arg2)) as u64 }
    (VAL_I8, CMP_LT)  => {((arg1 as i8) < (arg2 as i8)) as u64 }
    (VAL_I16, CMP_LT) => {((arg1 as i16) < (arg2 as i16)) as u64 }
    (VAL_I32, CMP_LT) => {((arg1 as i32) < (arg2 as i32)) as u64 }
    (VAL_I64, CMP_LT) => {((arg1 as i64) < (arg2 as i64)) as u64 }
    (VAL_F32, CMP_LT) => {(f32::from_bits(arg1 as u32) < f32::from_bits(arg2 as u32)) as u64 }
    (VAL_F64, CMP_LT) => {(f64::from_bits(arg1) < f64::from_bits(arg2)) as u64 }
    (VAL_I8, CMP_LE)  => {((arg1 as i8) <= (arg2 as i8)) as u64 }
    (VAL_I16, CMP_LE) => {((arg1 as i16) <= (arg2 as i16)) as u64 }
    (VAL_I32, CMP_LE) => {((arg1 as i32) <= (arg2 as i32)) as u64 }
    (VAL_I64, CMP_LE) => {((arg1 as i64) <= (arg2 as i64)) as u64 }
    (VAL_F32, CMP_LE) => {(f32::from_bits(arg1 as u32) <= f32::from_bits(arg2 as u32)) as u64 }
    (VAL_F64, CMP_LE) => {(f64::from_bits(arg1) <= f64::from_bits(arg2)) as u64 }
    (VAL_I8, CMP_ULT)  => {((arg1 as u8) < (arg2 as u8)) as u64 }
    (VAL_I16, CMP_ULT) => {((arg1 as u16) < (arg2 as u16)) as u64 }
    (VAL_I32, CMP_ULT) => {((arg1 as u32) < (arg2 as u32)) as u64 }
    (VAL_I64, CMP_ULT) => {(arg1 < arg2) as u64 }
    (VAL_F32, CMP_ULT) => {(!(f32::from_bits(arg1 as u32) >= f32::from_bits(arg2 as u32))) as u64 }
    (VAL_F64, CMP_ULT) => {(!(f64::from_bits(arg1) >= f64::from_bits(arg2))) as u64 }
    (VAL_I8, CMP_ULE)  => {((arg1 as u8) <= (arg2 as u8)) as u64 }
    (VAL_I16, CMP_ULE) => {((arg1 as u16) <= (arg2 as u16)) as u64 }
    (VAL_I32, CMP_ULE) => {((arg1 as u32) <= (arg2 as u32)) as u64 }
    (VAL_I64, CMP_ULE) => {(arg1 <= arg2) as u64 }
    (VAL_F32, CMP_ULE) => {(!(f32::from_bits(arg1 as u32) > f32::from_bits(arg2 as u32))) as u64 }
    (VAL_F64, CMP_ULE) => {(!(f64::from_bits(arg1) > f64::from_bits(arg2))) as u64 }
    _ => panic!("unsupported combination of val_type and compare_type: ({},{})",val_type,cmp_type)
  }
}
fn op_add(arg1: u64,arg2: u64,val_type: u32) -> u64 {
  match val_type {
    VAL_I8  => {((arg1 as u8) + (arg2 as u8)) as u64 }
    VAL_I16 => {((arg1 as u16) + (arg2 as u16)) as u64 }
    VAL_I32  => {((arg1 as u32) + (arg2 as u32)) as u64 }
    VAL_I64 => { arg1 + arg2 }
    VAL_F32 => {(f32::from_bits(arg1 as u32) + f32::from_bits(arg2 as u32)).to_bits() as u64 }
    VAL_F64 => {(f64::from_bits(arg1) + f64::from_bits(arg2)).to_bits()}
    _ => panic!("unsupported val_type for add: {}",val_type)
  }
}
fn op_sub(arg1: u64,arg2: u64,val_type: u32) -> u64 {
  match val_type {
    VAL_I8  => {((arg1 as u8) - (arg2 as u8)) as u64 }
    VAL_I16 => {((arg1 as u16) - (arg2 as u16)) as u64 }
    VAL_I32  => {((arg1 as u32) - (arg2 as u32)) as u64 }
    VAL_I64 => { arg1 - arg2 }
    VAL_F32 => {(f32::from_bits(arg1 as u32) - f32::from_bits(arg2 as u32)).to_bits() as u64 }
    VAL_F64 => {(f64::from_bits(arg1) - f64::from_bits(arg2)).to_bits()}
    _ => panic!("unsupported val_type for sub: {}",val_type)
  }
}
fn op_mul(arg1: u64,arg2: u64,val_type: u32) -> u64 {
  match val_type {
    VAL_I8  => {((arg1 as u8) * (arg2 as u8)) as u64 }
    VAL_I16 => {((arg1 as u16) * (arg2 as u16)) as u64 }
    VAL_I32  => {((arg1 as u32) * (arg2 as u32)) as u64 }
    VAL_I64 => { arg1 * arg2 }
    VAL_F32 => {(f32::from_bits(arg1 as u32) * f32::from_bits(arg2 as u32)).to_bits() as u64 }
    VAL_F64 => {(f64::from_bits(arg1) * f64::from_bits(arg2)).to_bits()}
    _ => panic!("unsupported val_type for mul: {}",val_type)
  }
}
fn op_and(arg1: u64,arg2: u64,val_type: u32) -> u64 {
  match val_type {
    VAL_I8  => {((arg1 as u8) & (arg2 as u8)) as u64 }
    VAL_I16 => {((arg1 as u16) & (arg2 as u16)) as u64 }
    VAL_I32  => {((arg1 as u32) & (arg2 as u32)) as u64 }
    VAL_I64 => { arg1 & arg2 }
    _ => panic!("unsupported val_type for and: {}",val_type)
  }
}
fn op_or(arg1: u64,arg2: u64,val_type: u32) -> u64 {
  match val_type {
    VAL_I8  => {((arg1 as u8) | (arg2 as u8)) as u64 }
    VAL_I16 => {((arg1 as u16) | (arg2 as u16)) as u64 }
    VAL_I32  => {((arg1 as u32) | (arg2 as u32)) as u64 }
    VAL_I64 => { arg1 | arg2 }
    _ => panic!("unsupported val_type for or: {}",val_type)
  }
}
fn op_xor(arg1: u64,arg2: u64,val_type: u32) -> u64 {
  match val_type {
    VAL_I8  => {((arg1 as u8) ^ (arg2 as u8)) as u64 }
    VAL_I16 => {((arg1 as u16) ^ (arg2 as u16)) as u64 }
    VAL_I32  => {((arg1 as u32) ^ (arg2 as u32)) as u64 }
    VAL_I64 => { arg1 ^ arg2 }
    _ => panic!("unsupported val_type for xor: {}",val_type)
  }
}
fn op_shl(arg1: u64,arg2: u64,val_type: u32) -> u64 {
  match val_type {
    VAL_I8  => {((arg1 as u8) << (arg2 as u8)) as u64 }
    VAL_I16 => {((arg1 as u16) << (arg2 as u16)) as u64 }
    VAL_I32  => {((arg1 as u32) << (arg2 as u32)) as u64 }
    VAL_I64 => { arg1 << arg2 }
    _ => panic!("unsupported val_type for shl: {}",val_type)
  }
}
fn op_lshr(arg1: u64,arg2: u64,val_type: u32) -> u64 {
  match val_type {
    VAL_I8  => {((arg1 as u8) >> (arg2 as u8)) as u64 }
    VAL_I16 => {((arg1 as u16) >> (arg2 as u16)) as u64 }
    VAL_I32  => {((arg1 as u32) >> (arg2 as u32)) as u64 }
    VAL_I64 => { arg1 >> arg2 }
    _ => panic!("unsupported val_type for lshr: {}",val_type)
  }
}
fn op_ashr(arg1: u64,arg2: u64,val_type: u32) -> u64 {
  match val_type {
    VAL_I8  => {((arg1 as i8) >> (arg2 as i8)) as u64 }
    VAL_I16 => {((arg1 as i16) >> (arg2 as i16)) as u64 }
    VAL_I32  => {((arg1 as i32) >> (arg2 as i32)) as u64 }
    VAL_I64 => { ((arg1 as i64) >> (arg2 as i64)) as u64 }
    _ => panic!("unsupported val_type for ashr: {}",val_type)
  }
}
fn op_cvt(src: u64,src_type: u32,signed: bool,dst_type: u32) -> u64 {
  match (src_type,signed,dst_type) {
    // uint <-> uint
    (VAL_I8,false,VAL_I16)  => {src as u8 as u16 as u64 }
    (VAL_I8,false,VAL_I32)  => {src as u8 as u32 as u64 }
    (VAL_I8,false,VAL_I64)  => {src as u8 as u64 as u64 }
    (VAL_I16,false,VAL_I8)  => {src as u16 as u8 as u64 }
    (VAL_I16,false,VAL_I32) => {src as u16 as u32 as u64 }
    (VAL_I16,false,VAL_I64) => {src as u16 as u64 as u64 }
    (VAL_I32,false,VAL_I8)  => {src as u32 as u8 as u64 }
    (VAL_I32,false,VAL_I16) => {src as u32 as u16 as u64 }
    (VAL_I32,false,VAL_I64) => {src as u32 as u64 as u64 }
    (VAL_I64,false,VAL_I8)  => {src as u64 as u8 as u64 }
    (VAL_I64,false,VAL_I16) => {src as u64 as u16 as u64 }
    (VAL_I64,false,VAL_I64) => {src as u64 as u32 as u64 }
    // sint <-> sint
    (VAL_I8,true,VAL_I16)  => {src as i8 as i16 as u64 }
    (VAL_I8,true,VAL_I32)  => {src as i8 as i32 as u64 }
    (VAL_I8,true,VAL_I64)  => {src as i8 as i64 as u64 }
    (VAL_I16,true,VAL_I8)  => {src as i16 as i8 as u64 }
    (VAL_I16,true,VAL_I32) => {src as i16 as i32 as u64 }
    (VAL_I16,true,VAL_I64) => {src as i16 as i64 as u64 }
    (VAL_I32,true,VAL_I8)  => {src as i32 as i8 as u64 }
    (VAL_I32,true,VAL_I16) => {src as i32 as i16 as u64 }
    (VAL_I32,true,VAL_I64) => {src as i32 as i64 as u64 }
    (VAL_I64,true,VAL_I8)  => {src as i64 as i8 as u64 }
    (VAL_I64,true,VAL_I16) => {src as i64 as i16 as u64 }
    (VAL_I64,true,VAL_I64) => {src as i64 as i32 as u64 }
    // uint -> float
    (VAL_I8,false,VAL_F32)  => {(src as u8 as f32).to_bits() as u64 }
    (VAL_I8,false,VAL_F64)  => {(src as u8 as f64).to_bits() as u64 }
    (VAL_I16,false,VAL_F32) => {(src as u16 as f32).to_bits() as u64 }
    (VAL_I16,false,VAL_F64) => {(src as u16 as f64).to_bits() as u64 }
    (VAL_I32,false,VAL_F32) => {(src as u32 as f32).to_bits() as u64 }
    (VAL_I32,false,VAL_F64) => {(src as u32 as f64).to_bits() as u64 }
    (VAL_I64,false,VAL_F32) => {(src as u64 as f32).to_bits() as u64 }
    (VAL_I64,false,VAL_F64) => {(src as u64 as f64).to_bits() as u64 }
    // int -> float
    (VAL_I8,true,VAL_F32)  => {(src as i8 as f32).to_bits() as u64 }
    (VAL_I8,true,VAL_F64)  => {(src as i8 as f64).to_bits() as u64 }
    (VAL_I16,true,VAL_F32) => {(src as i16 as f32).to_bits() as u64 }
    (VAL_I16,true,VAL_F64) => {(src as i16 as f64).to_bits() as u64 }
    (VAL_I32,true,VAL_F32) => {(src as i32 as f32).to_bits() as u64 }
    (VAL_I32,true,VAL_F64) => {(src as i32 as f64).to_bits() as u64 }
    (VAL_I64,true,VAL_F32) => {(src as i64 as f32).to_bits() as u64 }
    (VAL_I64,true,VAL_F64) => {(src as i64 as f64).to_bits() as u64 }
    // float -> uint
    (VAL_F32,false,VAL_I8)  => {f32::from_bits(src as u32) as u8 as u64 }
    (VAL_F64,false,VAL_I8)  => {f64::from_bits(src as u64) as u8 as u64 }
    (VAL_F32,false,VAL_I16) => {f32::from_bits(src as u32) as u16 as u64 }
    (VAL_F64,false,VAL_I16) => {f64::from_bits(src as u64) as u16 as u64 }
    (VAL_F32,false,VAL_I32)  => {f32::from_bits(src as u32) as u32 as u64 }
    (VAL_F64,false,VAL_I32)  => {f64::from_bits(src as u64) as u32 as u64 }
    (VAL_F32,false,VAL_I64)  => {f32::from_bits(src as u32) as u64 }
    (VAL_F64,false,VAL_I64)  => {f64::from_bits(src as u64) as u64 }
    // float -> sint
    (VAL_F32,true,VAL_I8)  => {f32::from_bits(src as u32) as i8 as u64 }
    (VAL_F64,true,VAL_I8)  => {f64::from_bits(src as u64) as i8 as u64 }
    (VAL_F32,true,VAL_I16) => {f32::from_bits(src as u32) as i16 as u64 }
    (VAL_F64,true,VAL_I16) => {f64::from_bits(src as u64) as i16 as u64 }
    (VAL_F32,true,VAL_I32)  => {f32::from_bits(src as u32) as i32 as u64 }
    (VAL_F64,true,VAL_I32)  => {f64::from_bits(src as u64) as i32 as u64 }
    (VAL_F32,true,VAL_I64)  => {f32::from_bits(src as u32) as i64 as u64 }
    (VAL_F64,true,VAL_I64)  => {f64::from_bits(src as u64) as i64 as u64 }
    // float -> float
    (VAL_F32,false,VAL_F64)  => {(f32::from_bits(src as u32) as f64).to_bits() as u64 }
    (VAL_F64,false,VAL_F32)  => {(f64::from_bits(src as u64) as f32).to_bits() as u64 }
    _ => panic!("unsupported val_types for {} conversion {} {}",
        if signed {"signed"}else{"unsigned"},
    src_type,dst_type)
  }
}


fn run(program: &mut Program) {
    let mut ip: usize = 0;
    let mut val_stack: Vec<u64> = Vec::new();
    let mut prog_stack: Vec<u64> = Vec::new();
    let mut rbp: usize = 0;
    while ip < program.code.len() {
      let op = program.code[ip];
      ip += 1;
      let op_type = op & 0xff;
      let op_data = op >> 8;
      let base_shift = 8; // how much has op-data been shifted
      match op_type {
        0x0..=0x7 => { // load-immediate[shift:3] [dst:4][data:*s]
          let dst = op_data & 0xf;
          let op_data = (op as i32) >> (base_shift+4);
          // sign-extend 20-bit value in op_data
          let val = ((op_data as i64) << 8*op_type) as u64; // shift value by 0 to 56 bits
          if dst == 0 {
            val_stack.push(val);
          } else {
            let old_val = stack_get(&val_stack,dst as usize);
            let val = val | old_val & ((1 << 8*op_type)-1);
            stack_set(val,&mut val_stack,dst as usize);
          }
        }
        0x8..=0xB => { // local-geti [dst:4][offset:*u]
          let dst = op_data & 0xf;
          let op_data = op_data >> 4;
          let size = 1 << (op_type-8); // 1,2,4,8
          let addr = rbp + op_data as usize;
          let mut buf: [u64;1] = [0;1];
          buffer_get(u64_as_bytes(&prog_stack),addr,&mut u64_as_bytes_mut(&mut buf)[0..size]);
          stack_set(buf[0],&mut val_stack,dst as usize);
        }
        0xC..=0xF => { // local-seti [src:4][offset:*u]
          let src = op_data & 0xf;
          let op_data = op_data >> 4;
          let size = 1 << (op_type-12); // 1,2,4,8
          let addr = rbp + op_data as usize;
          let src_val = stack_get(&val_stack,src as usize + 1);
          let src_vals: [u64;1] = [src_val;1];
          buffer_set(u64_as_bytes_mut(&mut prog_stack),addr,&u64_as_bytes(&src_vals)[0..size]);
        }
        0x10 => { // local-addr [dst:4][offset:*u]
          let dst = op_data & 0xf;
          let op_data = op_data >> 4;
          let addr = local_addr_to_ptr(rbp + op_data as usize);
          stack_set(addr,&mut val_stack,dst as usize);
        }
        0x11 => { // local-alloc
          panic!("unimplemented: local-alloc");
        }
        0x12|0x13 => { // ptr-get/ptr-set [dst:4][val?4][size:2][offset:*u]
          let is_set = op_type == 0x13;
          let mut op_data = op_data;
          let dst = op_data & 0xf;
          op_data >>= 4;
          let mut buf: [u64;1] = [0;1];
          if is_set {
            let src = op_data & 0xf;
            op_data >>= 4;
              buf[0] = stack_get(&val_stack,src as usize + 1);
          }
          let size = 1 << op_data & 0x3; // 1,2,4,8
          op_data >>= 2;
          let ptr = stack_get(&val_stack,dst as usize + 1) + (op_data as u64);
          let addr = (ptr & PTR_VALUE_MASK) as usize;
          match ptr & PTR_TYPE_MASK {
            PTR_LOCAL => {
              if is_set {
                buffer_set(u64_as_bytes_mut(&mut prog_stack),addr,&u64_as_bytes(&buf)[0..size]);
              } else {
                buffer_get(u64_as_bytes(&prog_stack),addr,&mut u64_as_bytes_mut(&mut buf)[0..size]);
              }
            }
            PTR_RODATA => {
              if is_set {
                panic!("cannot write to read-only address: 0x{:x}",ptr);
              }
              buffer_get(u64_as_bytes(&*program.rodata),addr,&mut u64_as_bytes_mut(&mut buf)[0..size]);
            }
            PTR_RWDATA => {
              if is_set {
                buffer_set(u64_as_bytes_mut(&mut* program.rwdata),addr,&u64_as_bytes(&buf)[0..size]);
              } else {
                buffer_get(u64_as_bytes(&*program.rwdata),addr,&mut u64_as_bytes_mut(&mut buf)[0..size]);
              }
            }
            PTR_HEAP => {
              panic!("heap pointers are not supported 0x{:x}",ptr)
            }
            _ => panic!("unknown pointer type 0x{:x}",ptr)
          }
        }
        // 0x14-0x1f
        // stack-mod [drop:4][count:24]
        // stack-mod [removeAt:4][count:4] [loc:16]
        // stack-mod [remove2:4][count1:2][count2:2] [loc1:8][loc2:8]
        // stack-mod [remove3:4] [loc1:6][loc2:6][loc3:6]
        // stack-mod [insert:4] [src:4] [dst:16]
        // stack-mod [insert-drop:4] [src:4] [dst:16]
        // stack-mod [extract:4] [dst:4] [src:16]
        // stack-mod [copyFrom:4][dst:4] [src:16]
        // stack-mod [copyTo:4][src:4] [dst:16]
        // stack-mod [copy2:4][drop:4] [A1][B1][A2][B2]
        // stack-mod [swap:4]  [A:4][B:16]
        // stack-mod [swap2:4] [A1:4][B1:4][A2:6][B2:6]
        // copy3 [A1][B1][C1][A2][B2][C2]
        // swap3 [A1][B1][C1][A2][B2][C2]
        0x20..=0x2f => { // jump/call[offset:24s], ret, jz/jnz [src:4][offset:20s]
          const JUMP_TYPE_JMP_ABS: u32 = 0;
          const JUMP_TYPE_CALL_ABS: u32 = 1;
          const JUMP_TYPE_JMP: u32 = 2;
          const JUMP_TYPE_CALL: u32 = 3;
          const JUMP_UNARY: u32 = 4; // start of unary jumps
          const JUMP_TYPE_JNZ: u32 = 4;
          const JUMP_TYPE_JNZ_DROP: u32 = 5;
          const JUMP_TYPE_JZ: u32 = 6;
          const JUMP_TYPE_JZ_DROP: u32 = 7;
          let jump_type = op_type & 0x7;
          let long_jump = (op_type & 0x8) != 0;
          // signed for relative jump, unsigned for absolute jump
          let mut op_data = if long_jump || jump_type <= JUMP_TYPE_CALL_ABS {
            op_data as i32 // unsigned immediate (high bit of op_data is zero)
          } else {
            (op as i32) >> base_shift // signed immediate (keep high bit of op)
          }; 
          let base_addr = if long_jump {
            val_stack.pop().unwrap()
          } else { 0 };
          let src = if jump_type < JUMP_UNARY { 0 } else {
            let index = op_data & 0xf;
            op_data >>= 4;
            stack_get(&val_stack,index as usize + 1)
          };
          let addr = base_addr as i64 + op_data as i64;
          match jump_type {
            JUMP_TYPE_JMP => {
              ip = (ip as i64 + addr) as usize;
            }
            JUMP_TYPE_JMP_ABS => {
              ip = addr as usize;
            }
            JUMP_TYPE_CALL | JUMP_TYPE_CALL_ABS => {
              let dst = if jump_type == JUMP_TYPE_CALL_ABS { addr as usize } else {
                (ip as i64 + addr) as usize
              };
              if addr == -1 { // return (call to -1 is endless loop)
                rbp = prog_stack.pop().unwrap() as usize;
                ip = prog_stack.pop().unwrap() as usize;
              } else {
                prog_stack.push(ip as u64);
                prog_stack.push(rbp as u64);
                rbp = 8*prog_stack.len();
                ip = dst;
              }
            }
            JUMP_TYPE_JNZ => {
              if src != 0 {
                ip = (ip as i64 + addr) as usize;
              }
            }
            JUMP_TYPE_JNZ_DROP => {
              val_stack.pop();
              if src != 0 {
                ip = (ip as i64 + addr) as usize;
              }
            }
            JUMP_TYPE_JZ => {
              if src == 0 {
                ip = (ip as i64 + addr) as usize;
              }
            }
            JUMP_TYPE_JZ_DROP => {
              val_stack.pop();
              if src == 0 {
                ip = (ip as i64 + addr) as usize;
              }
            }
            _ => panic!("unknown jump-type {}",jump_type)
          }
        }
        0x30..=0x37 => { // cmpi[val-type:3] [dst:4][src1:4][swap: 1][cmp-type:3][imm:12]
          let val_type = (op_type & 0x7) as u32; // i8 i16 i32 i64 . f16 f32 f64
          let op_data = (op as i32) >> base_shift;
          let dst = op_data & 0xf;
          let op_data = op_data >> 4;
          let src1 = op_data & 0xf;
          let op_data = op_data >> 4;
          let swap_args = (op_data & 0x8) != 0;
          let cmp_type = (op_data & 0x7) as u32; // eq ne . . lt le ult ule
          let mut op_data = op_data >> 4;
          if val_type >= VAL_FLOAT { // float -> immediate in high bits
            let bit_count = 1 << (val_type&0x3);
            if bit_count > 12 {
                op_data <<= bit_count - 12;
            }
          }
          let res = if swap_args {
            op_cmp(op_data as i64 as u64,stack_get(&val_stack,src1 as usize + 1),cmp_type as u32,val_type)
          } else {
            op_cmp(stack_get(&val_stack,src1 as usize + 1),op_data as i64 as u64,cmp_type,val_type)
          };
          stack_set(res,&mut val_stack,dst as usize)
        }
        0x38..=0x3f => { // addi[val-type:3] [dst:4][src1:4][imm:16]
          let val_type = op_type & 0x7; // i8 i16 i32 i64 . f16 f32 f64
          let op_data = (op as i32) >> base_shift;
          let dst = op_data & 0xf;
          let op_data = op_data >> 4;
          let src1 = op_data & 0xf;
          let mut op_data = op_data >> 4;
          if val_type >= VAL_FLOAT { // float -> immediate in high bits
            let bit_count = 1 << (val_type&0x3);
            if bit_count > 16 {
                op_data <<= bit_count - 16;
            }
          }
          let res = op_add(stack_get(&val_stack,src1 as usize + 1),op_data as i64 as u64,val_type);
          stack_set(res,&mut val_stack,dst as usize)
        }
        0x40..=0x47 => { // muli[val-type:3] [dst:4][src1:4][imm:16]
          let val_type = op_type & 0x7; // i8 i16 i32 i64 . f16 f32 f64
          let op_data = (op as i32) >> base_shift;
          let dst = op_data & 0xf;
          let op_data = op_data >> 4;
          let src1 = op_data & 0xf;
          let mut op_data = op_data >> 4;
          if val_type >= VAL_FLOAT { // float -> immediate in high bits
            let bit_count = 1 << (val_type&0x3);
            if bit_count > 16 {
                op_data <<= bit_count - 16;
            }
          }
          let res = op_mul(stack_get(&val_stack,src1 as usize + 1),op_data as i64 as u64,val_type);
          stack_set(res,&mut val_stack,dst as usize)
        }
        0x48..=0x4b => { // andi[val-type:2] [dst:4][src1:4][imm:16]
          let val_type = op_type & 0x3; // i8 i16 i32 i64
          let op_data = (op as i32) >> base_shift;
          let dst = op_data & 0xf;
          let op_data = op_data >> 4;
          let src1 = op_data & 0xf;
          let op_data = op_data >> 4;
          let res = op_and(stack_get(&val_stack,src1 as usize + 1),op_data as i64 as u64,val_type);
          stack_set(res,&mut val_stack,dst as usize)
        }
        0x4c..=0x4f => { // ori[val-type:2] [dst:4][src1:4][imm:16]
          let val_type = op_type & 0x3; // i8 i16 i32 i64
          let op_data = (op as i32) >> base_shift;
          let dst = op_data & 0xf;
          let op_data = op_data >> 4;
          let src1 = op_data & 0xf;
          let op_data = op_data >> 4;
          let res = op_or(stack_get(&val_stack,src1 as usize + 1),op_data as i64 as u64,val_type);
          stack_set(res,&mut val_stack,dst as usize)
        }
        0x50 => { // binary-op [dst:4][src1:4][src2:4][bin_op:4][cmp-type:3][val-type:3]
          let dst = op_data & 0xf;
          let op_data = op_data >> 4;
          let src1 = op_data & 0xf;
          let op_data = op_data >> 4;
          let src2 = op_data & 0xf;
          let op_data = op_data >> 4;
          let bin_op = op_data & 0xf;
          const OP_CMP: u32 = 0;
          const OP_ADD: u32 = 1;
          const OP_SUB: u32 = 2;
          const OP_MUL: u32 = 3;
          const OP_AND: u32 = 8;
          const OP_OR: u32 = 9;
          const OP_XOR: u32 = 10;
          const OP_SHL: u32 = 11;
          const OP_LSHR: u32 = 12;
          const OP_ASHR: u32 = 13;
          let op_data = op_data >> 4;
          let val_type = op_data & 0x7; // i8 i16 i32 i64 . f16 f32 f64
          let op_data = op_data >> 3;
          let res = match bin_op {
            OP_CMP => {
              let cmp_type = op_data & 0x7; // eq ne . . lt le ult ule
              op_cmp(
                stack_get(&val_stack,src1 as usize + 1),
                stack_get(&val_stack,src2 as usize + 1),
              cmp_type,val_type)
            }
            OP_ADD => {
              op_add(
                stack_get(&val_stack,src1 as usize + 1),
                stack_get(&val_stack,src2 as usize + 1),
              val_type)
            }
            OP_SUB => {
              op_sub(
                stack_get(&val_stack,src1 as usize + 1),
                stack_get(&val_stack,src2 as usize + 1),
              val_type)
            }
            OP_MUL => {
              op_mul(
                stack_get(&val_stack,src1 as usize + 1),
                stack_get(&val_stack,src2 as usize + 1),
              val_type)
            }
            // TODO: div/rem/udiv/urem
            OP_AND => {
              op_and(
                stack_get(&val_stack,src1 as usize + 1),
                stack_get(&val_stack,src2 as usize + 1),
              val_type)
            }
            OP_OR => {
              op_or(
                stack_get(&val_stack,src1 as usize + 1),
                stack_get(&val_stack,src2 as usize + 1),
              val_type)
            }
            OP_XOR => {
              op_xor(
                stack_get(&val_stack,src1 as usize + 1),
                stack_get(&val_stack,src2 as usize + 1),
              val_type)
            }
            OP_SHL => {
              op_shl(
                stack_get(&val_stack,src1 as usize + 1),
                stack_get(&val_stack,src2 as usize + 1),
              val_type)
            }
            OP_LSHR => {
              op_lshr(
                stack_get(&val_stack,src1 as usize + 1),
                stack_get(&val_stack,src2 as usize + 1),
              val_type)
            }
            OP_ASHR => {
              op_ashr(
                stack_get(&val_stack,src1 as usize + 1),
                stack_get(&val_stack,src2 as usize + 1),
              val_type)
            }
            _ => {panic!("unknown binary operation: {}",bin_op)}
          };
          stack_set(res,&mut val_stack,dst as usize)
        }
        0x51 => { // unary-op [dst:4][src:4][un_op:4][val-type:3]
          let dst = op_data & 0xf;
          let op_data = op_data >> 4;
          let src = op_data & 0xf;
          let op_data = op_data >> 4;
          let un_op = op_data & 0xf;
          let op_data = op_data >> 4;
          let val_type = op_data & 0x7; // i8 i16 i32 i64 . f16 f32 f64
          let op_data = op_data >> 3;
          const OP_NEG: u32 = 0;
          const OP_NOT: u32 = 1;
          const OP_SHLI: u32 = 2;
          const OP_LSHRI: u32 = 3;
          const OP_ASHRI: u32 = 4;
          let res = match un_op {
            OP_NEG => {
              op_sub(
                0,
                stack_get(&val_stack,src as usize + 1),
              val_type)
            }
            OP_NOT => {
              op_xor(
                stack_get(&val_stack,src as usize + 1),
                !0,
              val_type)
            }
            OP_SHLI => {
              op_shl(
                stack_get(&val_stack,src as usize + 1),
                op_data as u64,
              val_type)
            }
            OP_LSHRI => {
              op_lshr(
                stack_get(&val_stack,src as usize + 1),
                op_data as u64,
              val_type)
            }
            OP_ASHRI => {
              op_ashr(
                stack_get(&val_stack,src as usize + 1),
                op_data as u64,
              val_type)
            }
            _ => {panic!("unknown unary operation: {}",un_op)}
          };
          stack_set(res,&mut val_stack,dst as usize)
        }
        0x52 => { // cvt [dst:4][src:4][src-type:4][dst-type:4]
          let dst = op_data & 0xf;
          let op_data = op_data >> 4;
          let src = op_data & 0xf;
          let op_data = op_data >> 4;
          let signed = (op_data & 0x8) != 0;
          let dst_type = op_data & 0x7;
          let op_data = op_data >> 4;
          let src_type = op_data & 0x7;
          let res = op_cvt(stack_get(&val_stack,src as usize + 1),src_type,signed,dst_type);
          stack_set(res,&mut val_stack,dst as usize)
        }
        _ => panic!("unknown op-code 0x{:x}",op_type),
      }
    }
}

fn load_file(file: &mut File) -> Option<Program> {
  let mut header_buf: [u64; 4] = [0; 4]; // [version][code-size][ro-data-size][rw-data-size]
  file.read_exact(u64_as_bytes_mut(&mut header_buf)).ok()?;
  let _version = header_buf[0];
  let code_size = header_buf[1];
  if (code_size & 1) != 0 { panic!("code_size should be a multiple of 2")}
  let ro_data_size = header_buf[2];
  if (ro_data_size % 8) != 0 { panic!("ro_data_size should be a multiple of 8")}
  let rw_data_size = header_buf[3];
  if (rw_data_size % 8) != 0 { panic!("rw_data_size should be a multiple of 8")}
  let mut code_buffer = unsafe{ Box::<[u32]>::new_uninit_slice(code_size as usize).assume_init() };
  file.read_exact(u32_as_bytes_mut(&mut code_buffer)).ok()?;
  let mut ro_buffer = unsafe{ Box::<[u64]>::new_uninit_slice((ro_data_size/8) as usize).assume_init() };
  file.read_exact(u64_as_bytes_mut(&mut ro_buffer)).ok()?;
  let mut rw_buffer = unsafe{ Box::<[u64]>::new_uninit_slice((rw_data_size/8) as usize).assume_init() };
  file.read_exact(u64_as_bytes_mut(&mut rw_buffer)).ok()?;
  return Some(Program{code: code_buffer,rodata: ro_buffer,rwdata: rw_buffer})
}

fn main() -> io::Result<()> {
  let mut file = File::open("in.cctbc")?;
  // Read the content of the input file
  let Some(mut program) = load_file(&mut file) else {
    return Err(io::Error::new(io::ErrorKind::InvalidInput,"Failed to read File"))
  };
  run(&mut program);
  Ok(())
}
