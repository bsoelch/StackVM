use std::fs::File;
use std::io::Read;
use std::io;
use std::ptr;
use std::process;
use std::io::Write;

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
fn u64_as_bytes_box(ints: Box<[u64]>) -> Box<[u8]> {
  unsafe {
    let byte_count = ints.len()*8;
    let ptr = Box::into_raw(ints) as *mut u8;
    Box::from_raw(ptr::slice_from_raw_parts_mut(ptr, byte_count))
  }
}
fn u32_as_bytes(ints: &[u32]) -> &[u8] {
  unsafe {
    let ptr = ints.as_ptr() as *const u8;
    let len = ints.len() * 4;
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum AllocationType { STATIC, DYNAMIC }
#[derive(Debug, Clone, PartialEq)]
struct Allocation {
  start: u64,
  data: Box<[u8]>,
  readable: bool,
  writeable: bool,
  executable: bool,
  allocation_type: AllocationType
}
struct AllocationNode {
  nodes: Box<[TreeElement; 256]>,
  count: u64
}
enum TreeElement{
  Empty,
  Leaf(Box<Allocation>),
  Node(Box<AllocationNode>),
}
struct AllocationTree {
  root: AllocationNode,
  // TODO? caching for commonly accessed addressed (+ special-casing for code access)
}
impl AllocationNode{
  fn new() -> Self {
    AllocationNode{
      nodes: Box::new([const{TreeElement::Empty}; 256]),
      count: 0
    }
  }
}
impl AllocationTree{
  fn new() -> Self {
    AllocationTree{
      root: AllocationNode::new()
    }
  }
}
fn new_fixed_allocation_rec(node: &mut AllocationNode,allocation: &Allocation,key_shift: u32) {
  let key = (allocation.start >> key_shift) & 0xff;
  match &mut node.nodes[key as usize] {
    TreeElement::Empty => {
      node.nodes[key as usize] = TreeElement::Leaf(Box::new(allocation.clone()));
    },
    TreeElement::Leaf(prev_alloc) => {
      let prev_end = prev_alloc.start + (prev_alloc.data.len() as u64);
      let alloc_end = allocation.start + (allocation.data.len() as u64);
      if (prev_alloc.start <= allocation.start && prev_end > allocation.start) ||
         (allocation.start <= prev_alloc.start && alloc_end > prev_alloc.start) {
        panic!("overlapping allocations: {}:{} and {}:{}",prev_alloc.start,prev_end,allocation.start,alloc_end);
      }
      let mut sub_tree = Box::new(AllocationNode::new());
      new_fixed_allocation_rec(&mut sub_tree,&prev_alloc,key_shift-8);
      new_fixed_allocation_rec(&mut sub_tree,allocation,key_shift-8);
      node.nodes[key as usize] = TreeElement::Node(sub_tree);
    }
    TreeElement::Node(sub_tree) => {
      new_fixed_allocation_rec(sub_tree,allocation,key_shift-8);
    }
  }
}
fn new_fixed_allocation(tree: &mut AllocationTree,allocation: &Allocation) {
  if (allocation.start % 16) != 0 {panic!("allocations have to be alligned to 16-bytes")}
  if allocation.data.len() == 0 {return} // nothing to do
  new_fixed_allocation_rec(&mut tree.root,allocation,56);
}
// TODO? mmap, mremap, munmap
// TODO? memcopy, memset
fn read_data_rec(node: &AllocationNode,addr: u64,dst: &mut[u8],key_shift: u32) {
  let key = (addr >> key_shift) & 0xff;
  match &node.nodes[key as usize] {
    TreeElement::Empty => panic!("invalid memory access"),
    TreeElement::Leaf(alloc) => {
      if addr < alloc.start {panic!("invalid memory access")}
      let local_addr = (addr - alloc.start) as usize;
      if local_addr + dst.len() > alloc.data.len() {panic!("invalid memory access")}
      if !alloc.readable {panic!("invalid memory access")}
      dst.copy_from_slice(&alloc.data[local_addr..(local_addr+dst.len())])
    }
    TreeElement::Node(sub_tree) => {
      read_data_rec(sub_tree,addr,dst,key_shift-8);
    }
  }
}
fn write_data_rec(node: &mut AllocationNode,addr: u64,src: &[u8],key_shift: u32) {
  let key = (addr >> key_shift) & 0xff;
  match &mut node.nodes[key as usize] {
    TreeElement::Empty => panic!("invalid memory access"),
    TreeElement::Leaf(alloc) => {
      if addr < alloc.start {panic!("invalid memory access")}
      let local_addr = (addr - alloc.start) as usize;
      if local_addr + src.len() > alloc.data.len() {panic!("invalid memory access")}
      if !alloc.writeable {panic!("invalid memory access")}
      alloc.data[local_addr..(local_addr+src.len())].copy_from_slice(src)
    }
    TreeElement::Node(sub_tree) => {
      write_data_rec(sub_tree,addr,src,key_shift-8);
    }
  }
}
fn read_data(tree: &AllocationTree,addr: u64,dst: &mut[u8]) {
  read_data_rec(&tree.root,addr,dst,56);
}
fn write_data(tree: &mut AllocationTree,addr: u64,src: &[u8]) {
  write_data_rec(&mut tree.root,addr,src,56);
}

struct Program{
  code: Box<[u32]>, // TODO: move code to main memory space (+ caching of current code allocation)
  ip: u64,
  sp: u64,
  ro_addr: u64,
  rw_addr: u64,
  allocations: AllocationTree,
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
fn prog_stack_pop(program: &mut Program) -> u64 {
  let mut data: [u64; 1] = [0; 1];
  read_data(&program.allocations,program.sp,u64_as_bytes_mut(&mut data));
  program.sp += 8;
  return data[0];
}
fn prog_stack_push(program: &mut Program,value: u64) {
  let data: [u64; 1] = [value; 1];
  program.sp -= 8;
  write_data(&mut program.allocations,program.sp,u64_as_bytes(&data));
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
    VAL_I8  => {((arg1 as u8).wrapping_add(arg2 as u8)) as u64 }
    VAL_I16 => {((arg1 as u16).wrapping_add(arg2 as u16)) as u64 }
    VAL_I32  => {((arg1 as u32).wrapping_add(arg2 as u32)) as u64 }
    VAL_I64 => { arg1.wrapping_add(arg2) }
    VAL_F32 => {(f32::from_bits(arg1 as u32) + f32::from_bits(arg2 as u32)).to_bits() as u64 }
    VAL_F64 => {(f64::from_bits(arg1) + f64::from_bits(arg2)).to_bits()}
    _ => panic!("unsupported val_type for add: {}",val_type)
  }
}
fn op_sub(arg1: u64,arg2: u64,val_type: u32) -> u64 {
  match val_type {
    VAL_I8  => {((arg1 as u8).wrapping_sub(arg2 as u8)) as u64 }
    VAL_I16 => {((arg1 as u16).wrapping_sub(arg2 as u16)) as u64 }
    VAL_I32  => {((arg1 as u32).wrapping_sub(arg2 as u32)) as u64 }
    VAL_I64 => { arg1.wrapping_sub(arg2) }
    VAL_F32 => {(f32::from_bits(arg1 as u32) - f32::from_bits(arg2 as u32)).to_bits() as u64 }
    VAL_F64 => {(f64::from_bits(arg1) - f64::from_bits(arg2)).to_bits()}
    _ => panic!("unsupported val_type for sub: {}",val_type)
  }
}
fn op_mul(arg1: u64,arg2: u64,val_type: u32) -> u64 {
  match val_type {
    VAL_I8  => {((arg1 as u8).wrapping_mul(arg2 as u8)) as u64 }
    VAL_I16 => {((arg1 as u16).wrapping_mul(arg2 as u16)) as u64 }
    VAL_I32  => {((arg1 as u32).wrapping_mul(arg2 as u32)) as u64 }
    VAL_I64 => { arg1.wrapping_mul(arg2) }
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
    VAL_I8  => {((arg1 as u8).wrapping_shl(arg2 as u32)) as u64 }
    VAL_I16 => {((arg1 as u16).wrapping_shl(arg2 as u32)) as u64 }
    VAL_I32  => {((arg1 as u32).wrapping_shl(arg2 as u32)) as u64 }
    VAL_I64 => { arg1.wrapping_shl(arg2 as u32) }
    _ => panic!("unsupported val_type for shl: {}",val_type)
  }
}
fn op_lshr(arg1: u64,arg2: u64,val_type: u32) -> u64 {
  match val_type {
    VAL_I8  => {((arg1 as u8).wrapping_shr(arg2 as u32)) as u64 }
    VAL_I16 => {((arg1 as u16).wrapping_shr(arg2 as u32)) as u64 }
    VAL_I32  => {((arg1 as u32).wrapping_shr(arg2 as u32)) as u64 }
    VAL_I64 => { arg1.wrapping_shr(arg2 as u32) }
    _ => panic!("unsupported val_type for lshr: {}",val_type)
  }
}
fn op_ashr(arg1: u64,arg2: u64,val_type: u32) -> u64 {
  match val_type {
    VAL_I8  => {((arg1 as i8).wrapping_shr(arg2 as u32)) as u64 }
    VAL_I16 => {((arg1 as i16).wrapping_shr(arg2 as u32)) as u64 }
    VAL_I32  => {((arg1 as i32).wrapping_shr(arg2 as u32)) as u64 }
    VAL_I64 => { ((arg1 as i64).wrapping_shr(arg2 as u32)) as u64 }
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

const TRACE:bool = true;
fn val_type_name(val_type: u32) -> &'static str {
  match val_type {
    VAL_I8  => "i8",
    VAL_I16 => "i16",
    VAL_I32 => "i32",
    VAL_I64 => "i64",
    VAL_F16 => "f16",
    VAL_F32 => "f32",
    VAL_F64 => "f64",
    _ => panic!("unsupported val_type: {}",val_type)
  }
}
fn cmp_type_name(cmp_type: u32) -> &'static str {
  match cmp_type {
    CMP_EQ  => "eq",
    CMP_NE  => "ne",
    CMP_LT  => "lt",
    CMP_LE  => "le",
    CMP_ULT => "ult",
    CMP_ULE => "ule",
    _ => panic!("unsupported cmp_type: {}",cmp_type)
  }
}

fn run(program: &mut Program) {
    let mut ip: usize = program.ip as usize;
    let mut val_stack: Vec<u64> = Vec::new();
    let mut rbp: u64 = program.sp;
    while ip < program.code.len() {
      let op = program.code[ip];
      ip += 1;
      let op_type = op & 0xff;
      let op_data = op >> 8;
      let base_shift = 8; // how much has op-data been shifted
      if TRACE {println!("{:09}: {:08x}  {:?}",ip-1,op,val_stack);}
      match op_type {
        0x00..=0x0b => { // load-immediate[shift:3] [dst:4][data:*s]
          let dst = op_data & 0xf;
          let op_data = (op as i32) >> (base_shift+4);
          // sign-extend 20-bit value in op_data
          let val = ((op_data as i64) << 4*op_type) as u64; // shift value by 0 to 44 bits
          if TRACE {println!("loadi.{} @{} ${}",op_type*4,dst,op_data);}
          if dst == 0 {
            val_stack.push(val);
          } else {
            let old_val = stack_get(&val_stack,dst as usize);
            let val = val | old_val & ((1 << 8*op_type)-1);
            stack_set(val,&mut val_stack,dst as usize);
          }
        }
        0x0c => { // local-addr [dst:4][offset:20u]
          let dst = (op_data & 0xf) as usize;
          let offset = (op_data >> 4) as u64;
          stack_set(rbp + offset,&mut val_stack,dst);
          if TRACE {println!("addr.local @{} ${}",dst,offset);}
        }
        0x0d => { // ip-relative-addr [dst:4][offset:20s]
          let op_data = (op as i32) >> (base_shift+4);
          let dst = (op_data & 0xf) as usize;
          let offset = (op_data >> 4) as i64;
          stack_set((ip as i64 + offset) as u64,&mut val_stack,dst);
          if TRACE {println!("addr.ip @{} ${}",dst,offset);}
        }
        0x0e => { // ro-addr [dst:4][offset:20u]
          let dst = (op_data & 0xf) as usize;
          let offset = (op_data >> 4) as u64;
          stack_set(program.ro_addr + offset,&mut val_stack,dst);
          if TRACE {println!("addr.ro @{} ${}",dst,offset);}
        }
        0x0f => { // rw-addr [dst:4][offset:20u]
          let dst = (op_data & 0xf) as usize;
          let offset = (op_data >> 4) as u64;
          stack_set(program.rw_addr + offset,&mut val_stack,dst);
          if TRACE {println!("addr.rw @{} ${}",dst,offset);}
        }
        0x10|0x11 => { // load/store [dst:4][src:4][size:2][offset:14u]
          let is_store = op_type == 0x11;
          let dst = (op_data & 0xf) as usize;
          let op_data = op_data >> 4;
          let mut buf: [u64;1] = [0;1];
          let ptr = (op_data & 0xf) as usize + 1; // needed for tracing
          let op_data = op_data >> 4;
          if is_store {
            buf[0] = stack_get(&val_stack,dst + 1);
          }
          let size = 1 << (op_data & 0x3) as usize; // 1,2,4,8
          let op_data = op_data >> 2;
          let addr = stack_get(&val_stack,ptr) + (op_data as u64);
          if TRACE {
            if is_store {
                println!("store.{} @{} @{}+{}",size,dst,ptr,op_data);
            } else {
                println!("load.{} @{} @{}+{}",size,dst,ptr,op_data);
            }
          }
          if is_store {
            write_data(&mut program.allocations,addr,&u64_as_bytes(&buf)[0..size]);
          } else {
            read_data(&program.allocations,addr,&mut u64_as_bytes_mut(&mut buf)[0..size]);
            stack_set(buf[0],&mut val_stack,dst);
          }
        }
        0x12 => { // load.local [dst:4][size:2][offset:18u]
          let dst = (op_data & 0xf) as usize;
          let op_data = op_data >> 4;
          let size = 1 << (op_data&0x3) as usize; // 1,2,4,8
          let offset = (op_data >> 2) as u64;
          let addr = rbp + offset;
          if TRACE {println!("load.{} @{} @bp+{}",size,dst,offset);}
          let mut buf: [u64;1] = [0;1];
          read_data(&program.allocations,addr,&mut u64_as_bytes_mut(&mut buf)[0..size]);
          stack_set(buf[0],&mut val_stack,dst);
        }
        0x13 => { // store.local [src:4][size:2][offset:18u]
          let src = (op_data & 0xf) as usize + 1;
          let op_data = op_data >> 4;
          let size = 1 << (op_data&0x3) as usize; // 1,2,4,8
          let offset = (op_data >> 2) as u64;
          let addr = rbp + offset;
          let src_val = stack_get(&val_stack,src);
          let src_vals: [u64;1] = [src_val;1];
          if TRACE {println!("store.{} @{} @bp+{}",size,src,offset);}
          write_data(&mut program.allocations,addr,&u64_as_bytes(&src_vals)[0..size]);
        }
        0x14 => { // load.ip [dst:4][size:2][offset:18s]
          let dst = (op_data & 0xf) as usize;
          let op_data = (op as i32) >> base_shift + 4;
          let size = 1 << (op_data&0x3) as usize; // 1,2,4,8
          let offset = (op_data >> 2) as i64;
          let addr = ((ip as i64) * 4 + offset) as usize;
          let mut buf: [u64;1] = [0;1];
          if TRACE {println!("load.{} @{} @ip{}{}",size,dst,if offset >= 0 {"+"}else{""},offset);}
          // TODO? will this lead to problems with aliasing
          u64_as_bytes_mut(&mut buf).copy_from_slice(&u32_as_bytes(&program.code)[addr..(addr+8)]);
          // read_data(&program.allocations,addr,&mut u64_as_bytes_mut(&mut buf)[0..size]);
          stack_set(buf[0],&mut val_stack,dst);
        }
        0x15 => { // load.ro [dst:4][size:2][offset:18u]
          let dst = (op_data & 0xf) as usize;
          let op_data = op_data >> 4;
          let size = 1 << (op_data&0x3) as usize; // 1,2,4,8
          let offset = (op_data >> 2) as u64;
          let addr = program.ro_addr + offset;
          if TRACE {println!("load.{} @{} @ro_data+{}",size,dst,offset);}
          let mut buf: [u64;1] = [0;1];
          read_data(&program.allocations,addr,&mut u64_as_bytes_mut(&mut buf)[0..size]);
          stack_set(buf[0],&mut val_stack,dst);
        }
        0x16 => { // load.rw [dst:4][size:2][offset:18u]
          let dst = (op_data & 0xf) as usize;
          let op_data = op_data >> 4;
          let size = 1 << (op_data&0x3) as usize; // 1,2,4,8
          let offset = (op_data >> 2) as u64;
          let addr = program.rw_addr + offset;
          if TRACE {println!("load.{} @{} @rw_data+{}",size,dst,offset);}
          let mut buf: [u64;1] = [0;1];
          read_data(&program.allocations,addr,&mut u64_as_bytes_mut(&mut buf)[0..size]);
          stack_set(buf[0],&mut val_stack,dst);
        }
        0x17 => { // store.rw [src:4][size:2][offset:18u]
          let src = (op_data & 0xf) as usize + 1;
          let op_data = op_data >> 4;
          let size = 1 << (op_data&0x3) as usize; // 1,2,4,8
          let offset = (op_data >> 2) as u64;
          let addr = program.rw_addr + offset;
          let src_val = stack_get(&val_stack,src);
          if TRACE {println!("store.{} @{} @rw_data+{}",size,src,offset);}
          let src_vals: [u64;1] = [src_val;1];
          write_data(&mut program.allocations,addr,&u64_as_bytes(&src_vals)[0..size]);
        }
        0x18|0x19 => { // load2/store2 [dst1:4][dst2:4][ptr:4][offset:12u]
          let is_store = op_type == 0x13;
          let dst1 = (op_data & 0xf) as usize;
          let op_data = op_data >> 4;
          let dst2 = (op_data & 0xf) as usize;
          let op_data = op_data >> 4;
          let mut buf: [u64;2] = [0;2];
          let ptr = (op_data & 0xf) as usize + 1; // needed for tracing
          let op_data = op_data >> 4;
          if is_store {
            buf[0] = stack_get(&val_stack,dst1 + 1);
            buf[1] = stack_get(&val_stack,dst2 + 1);
          }
          let addr = stack_get(&val_stack,ptr) + (op_data as u64);
          if TRACE {
            if is_store {
                println!("store2 @{} @{} @{}+{}",dst1,dst2,ptr,op_data);
            } else {
                println!("load2 @{} @{} @{}+{}",dst1,dst2,ptr,op_data);
            }
          }
          if is_store {
            write_data(&mut program.allocations,addr,u64_as_bytes(&buf));
          } else {
            read_data(&program.allocations,addr,u64_as_bytes_mut(&mut buf));
            stack_set(buf[0],&mut val_stack,dst1);
            stack_set(buf[1],&mut val_stack,dst2);
          }
        }
        0x1a => { // load2.local [dst1:4][dst2:4][offset:16u]
          let dst1 = (op_data & 0xf) as usize;
          let op_data = op_data >> 4;
          let dst2 = (op_data & 0xf) as usize;
          let offset = (op_data >> 4) as u64;
          let addr = rbp + offset;
          let mut buf: [u64;2] = [0;2];
          if TRACE {println!("load2 @{} @{} @bp+{}",dst1,dst2,op_data);}
          read_data(&program.allocations,addr,u64_as_bytes_mut(&mut buf));
          stack_set(buf[0],&mut val_stack,dst1);
          stack_set(buf[1],&mut val_stack,dst2);
        }
        0x1b => { // store2.local [src1:4][src2:4][offset:16u]
          let src1 = (op_data & 0xf) as usize + 1;
          let op_data = op_data >> 4;
          let src2 = (op_data & 0xf) as usize + 1;
          let offset = (op_data >> 4) as u64;
          let addr = rbp + offset;
          let src1_val = stack_get(&val_stack,src1);
          let src2_val = stack_get(&val_stack,src2);
          let src_vals: [u64;2] = [src1_val,src2_val];
          if TRACE {println!("store2 @{} @{} @bp+{}",src1,src2,op_data);}
          write_data(&mut program.allocations,addr,u64_as_bytes(&src_vals));
        }
        0x1c => { // load2.ip [dst1:4][dst2:4][offset:16s]
          let dst1 = (op_data & 0xf) as usize;
          let op_data = (op as i32) >> base_shift + 4;
          let dst2 = (op_data & 0xf) as usize;
          let offset = (op_data >> 4) as i64;
          let addr = ((ip as i64) * 4 + offset) as usize;
          if TRACE {println!("load2 @{} @{} @ip{}{}",dst1,dst2,if op_data >= 0 {"+"}else{""},offset);}
          let mut buf: [u64;2] = [0;2];
          u64_as_bytes_mut(&mut buf).copy_from_slice(&u32_as_bytes(&program.code)[addr..(addr+16)]);
          // read_data(&program.allocations,addr,&mut u64_as_bytes_mut(&mut buf)[0..size]);
          stack_set(buf[0],&mut val_stack,dst1);
          stack_set(buf[1],&mut val_stack,dst2);
        }
        0x1d => { // load2.ro [dst1:4][dst2:4][offset:16u]
          let dst1 = (op_data & 0xf) as usize;
          let op_data = op_data >> 4;
          let dst2 = (op_data & 0xf) as usize ;
          let offset = (op_data >> 4) as u64;
          let addr = program.ro_addr + offset;
          if TRACE {println!("load2 @{} @{} @ro_data+{}",dst1,dst2,op_data);}
          let mut buf: [u64;2] = [0;2];
          read_data(&program.allocations,addr,u64_as_bytes_mut(&mut buf));
          stack_set(buf[0],&mut val_stack,dst1);
          stack_set(buf[1],&mut val_stack,dst2);
        }
        0x1e => { // load2.rw [dst1:4][dst2:4][offset:16u]
          let dst1 = (op_data & 0xf) as usize;
          let op_data = op_data >> 4;
          let dst2 = (op_data & 0xf) as usize;
          let offset = (op_data >> 4) as u64;
          let addr = program.rw_addr + offset;
          if TRACE {println!("load2 @{} @{} @rw_data+{}",dst1,dst2,op_data);}
          let mut buf: [u64;2] = [0;2];
          read_data(&program.allocations,addr,u64_as_bytes_mut(&mut buf));
          stack_set(buf[0],&mut val_stack,dst1);
          stack_set(buf[1],&mut val_stack,dst2);
        }
        0x1f => { // store2.rw [src1:4][src2:4][offset:16u]
          let src1 = (op_data & 0xf) as usize + 1;
          let op_data = op_data >> 4;
          let src2 = (op_data & 0xf) as usize + 1;
          let offset = (op_data >> 4) as u64;
          let addr = program.rw_addr + offset;
          let src1_val = stack_get(&val_stack,src1);
          let src2_val = stack_get(&val_stack,src2);
          let src_vals: [u64;2] = [src1_val,src2_val];
          if TRACE {println!("store2 @{} @{} @rw_data+{}",src1,src2,op_data);}
          write_data(&mut program.allocations,addr,u64_as_bytes(&src_vals));
        }
        0x20..=0x2f => { // jump/call[offset:24s], ret, jz/jnz [src:4][offset:20s]
          const JUMP_TYPE_JMP_ABS: u32 = 0;
          const JUMP_TYPE_CALL_ABS: u32 = 1;
          const JUMP_TYPE_JMP: u32 = 2;
          const JUMP_TYPE_CALL: u32 = 3;
          const JUMP_UNARY: u32 = 4; // start of unary jumps
          const JUMP_TYPE_JNZ: u32 = 4;
          const JUMP_TYPE_JZ: u32 = 5;
          const JUMP_POP: u32 = 6; // start of dropping jumps
          const JUMP_TYPE_JNZ_POP: u32 = 6;
          const JUMP_TYPE_JZ_POP: u32 = 7;
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
          let src_index = (op_data & 0xf) as usize + 1; // needed for tracing
          let src = if jump_type < JUMP_UNARY { 0 }
          else if jump_type >= JUMP_POP {
            val_stack.pop().unwrap()
          } else {
            op_data >>= 4;
            stack_get(&val_stack,src_index)
          };
          let addr = base_addr as i64 + op_data as i64;
          match jump_type {
            JUMP_TYPE_JMP => {
              if TRACE {println!("jmp ${}",addr);}
              ip = (ip as i64 + addr) as usize;
            }
            JUMP_TYPE_JMP_ABS => {
              if TRACE {println!("jmp.abs ${}",addr);}
              ip = addr as usize;
            }
            JUMP_TYPE_CALL | JUMP_TYPE_CALL_ABS => {
              let dst = if jump_type == JUMP_TYPE_CALL_ABS {
                addr as usize
              } else {
                (ip as i64 + addr) as usize
              };
              if addr == -1 { // return (call to -1 is endless loop)
                if TRACE {println!("ret");}
                rbp = prog_stack_pop(program);
                ip = prog_stack_pop(program) as usize;
              } else {
                if TRACE {
                    if jump_type == JUMP_TYPE_CALL_ABS {
                        println!("call.abs ${}",addr);
                    } else {
                        println!("call ${}",addr);
                    }
                }
                prog_stack_push(program,ip as u64);
                prog_stack_push(program,rbp as u64);
                rbp = program.sp;
                ip = dst;
              }
            }
            JUMP_TYPE_JNZ => {
              if TRACE {println!("jnz @{} ${}",src_index,addr);}
              if src != 0 {
                ip = (ip as i64 + addr) as usize;
              }
            }
            JUMP_TYPE_JNZ_POP => {
              if TRACE {println!("jnz pop ${}",addr);}
              if src != 0 {
                ip = (ip as i64 + addr) as usize;
              }
            }
            JUMP_TYPE_JZ => {
              if TRACE {println!("jz @{} ${}",src_index,addr);}
              if src == 0 {
                ip = (ip as i64 + addr) as usize;
              }
            }
            JUMP_TYPE_JZ_POP => {
              if TRACE {println!("jz pop ${}",addr);}
              if src == 0 {
                ip = (ip as i64 + addr) as usize;
              }
            }
            _ => panic!("unknown jump-type {}",jump_type)
          }
        }
        0x30..=0x37 => { // cmpi[val-type:3] [dst:4][src1:4][cmp-type:3][swap: 1][imm:12]
          let val_type = (op_type & 0x7) as u32; // i8 i16 i32 i64 . f16 f32 f64
          let op_data = (op as i32) >> base_shift;
          let dst = (op_data & 0xf) as usize;
          let op_data = op_data >> 4;
          let src1 = (op_data & 0xf) as usize +1;
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
            if TRACE {println!("cmpi.{} {} @{} ${} @{}",val_type_name(val_type),cmp_type_name(cmp_type),dst,op_data,src1);}
            op_cmp(op_data as i64 as u64,stack_get(&val_stack,src1),cmp_type as u32,val_type)
          } else {
            if TRACE {println!("cmpi.{} {} @{} @{} ${}",val_type_name(val_type),cmp_type_name(cmp_type),dst,src1,op_data);}
            op_cmp(stack_get(&val_stack,src1),op_data as i64 as u64,cmp_type,val_type)
          };
          stack_set(res,&mut val_stack,dst)
        }
        0x38..=0x3f => { // addi[val-type:3] [dst:4][src1:4][imm:16]
          let val_type = op_type & 0x7; // i8 i16 i32 i64 . f16 f32 f64
          let op_data = (op as i32) >> base_shift;
          let dst = (op_data & 0xf) as usize;
          let op_data = op_data >> 4;
          let src1 = (op_data & 0xf) as usize +1;
          let mut op_data = op_data >> 4;
          if TRACE {println!("addi.{} @{} @{} ${}",val_type_name(val_type),dst,src1,op_data);}
          if val_type >= VAL_FLOAT { // float -> immediate in high bits
            let bit_count = 1 << (val_type&0x3);
            if bit_count > 16 {
                op_data <<= bit_count - 16;
            }
          }
          let res = op_add(stack_get(&val_stack,src1),op_data as i64 as u64,val_type);
          stack_set(res,&mut val_stack,dst)
        }
        0x40..=0x47 => { // muli[val-type:3] [dst:4][src1:4][imm:16]
          let val_type = op_type & 0x7; // i8 i16 i32 i64 . f16 f32 f64
          let op_data = (op as i32) >> base_shift;
          let dst = (op_data & 0xf) as usize;
          let op_data = op_data >> 4;
          let src1 = (op_data & 0xf) as usize +1;
          let mut op_data = op_data >> 4;
          if TRACE {println!("muli.{} @{} @{} ${}",val_type_name(val_type),dst,src1,op_data);}
          if val_type >= VAL_FLOAT { // float -> immediate in high bits
            let bit_count = 1 << (val_type&0x3);
            if bit_count > 16 {
                op_data <<= bit_count - 16;
            }
          }
          let res = op_mul(stack_get(&val_stack,src1),op_data as i64 as u64,val_type);
          stack_set(res,&mut val_stack,dst)
        }
        0x48..=0x4b => { // andi[val-type:2] [dst:4][src1:4][imm:16]
          let val_type = op_type & 0x3; // i8 i16 i32 i64
          let op_data = (op as i32) >> base_shift;
          let dst = (op_data & 0xf) as usize;
          let op_data = op_data >> 4;
          let src1 = (op_data & 0xf) as usize +1;
          let op_data = op_data >> 4;
          if TRACE {println!("andi.{} @{} @{} ${}",val_type_name(val_type),dst,src1,op_data);}
          let res = op_and(stack_get(&val_stack,src1),op_data as i64 as u64,val_type);
          stack_set(res,&mut val_stack,dst)
        }
        0x4c..=0x4f => { // ori[val-type:2] [dst:4][src1:4][imm:16]
          let val_type = op_type & 0x3; // i8 i16 i32 i64
          let op_data = (op as i32) >> base_shift;
          let dst = (op_data & 0xf) as usize;
          let op_data = op_data >> 4;
          let src1 = (op_data & 0xf) as usize +1;
          let op_data = op_data >> 4;
          if TRACE {println!("ori.{} @{} @{} ${}",val_type_name(val_type),dst,src1,op_data);}
          let res = op_or(stack_get(&val_stack,src1),op_data as i64 as u64,val_type);
          stack_set(res,&mut val_stack,dst)
        }
        0x50..=0x57 => { // binary-op[val-type:3] [bin_op:4][dst:4][src1:4][src2:4][cmp-type:3]
          let val_type = op_type & 0x7; // i8 i16 i32 i64 . f16 f32 f64
          let bin_op = op_data & 0xf;
          let op_data = op_data >> 4;
          let dst = (op_data & 0xf) as usize;
          let op_data = op_data >> 4;
          let src1 = (op_data & 0xf) as usize + 1;
          let op_data = op_data >> 4;
          let src2 = (op_data & 0xf) as usize + 1;
          let op_data = op_data >> 4;
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
          let res = match bin_op {
            OP_CMP => {
              let cmp_type = op_data & 0x7; // eq ne . . lt le ult ule
              if TRACE {println!("cmp.{} {} @{} @{} @{}",val_type_name(val_type),cmp_type_name(cmp_type),dst,src1,src2);}
              op_cmp(
                stack_get(&val_stack,src1),
                stack_get(&val_stack,src2),
              cmp_type,val_type)
            }
            OP_ADD => {
              if TRACE {println!("add.{} @{} @{} @{}",val_type_name(val_type),dst,src1,src2);}
              op_add(
                stack_get(&val_stack,src1),
                stack_get(&val_stack,src2),
              val_type)
            }
            OP_SUB => {
              if TRACE {println!("sub.{} @{} @{} @{}",val_type_name(val_type),dst,src1,src2);}
              op_sub(
                stack_get(&val_stack,src1),
                stack_get(&val_stack,src2),
              val_type)
            }
            OP_MUL => {
              if TRACE {println!("mul.{} @{} @{} @{}",val_type_name(val_type),dst,src1,src2);}
              op_mul(
                stack_get(&val_stack,src1),
                stack_get(&val_stack,src2),
              val_type)
            }
            // TODO: div/rem/udiv/urem
            OP_AND => {
              if TRACE {println!("and.{} @{} @{} @{}",val_type_name(val_type),dst,src1,src2);}
              op_and(
                stack_get(&val_stack,src1),
                stack_get(&val_stack,src2),
              val_type)
            }
            OP_OR => {
              if TRACE {println!("or.{} @{} @{} @{}",val_type_name(val_type),dst,src1,src2);}
              op_or(
                stack_get(&val_stack,src1),
                stack_get(&val_stack,src2),
              val_type)
            }
            OP_XOR => {
              if TRACE {println!("xor.{} @{} @{} @{}",val_type_name(val_type),dst,src1,src2);}
              op_xor(
                stack_get(&val_stack,src1),
                stack_get(&val_stack,src2),
              val_type)
            }
            OP_SHL => {
              if TRACE {println!("shl.{} @{} @{} @{}",val_type_name(val_type),dst,src1,src2);}
              op_shl(
                stack_get(&val_stack,src1),
                stack_get(&val_stack,src2),
              val_type)
            }
            OP_LSHR => {
              if TRACE {println!("lshr.{} @{} @{} @{}",val_type_name(val_type),dst,src1,src2);}
              op_lshr(
                stack_get(&val_stack,src1),
                stack_get(&val_stack,src2),
              val_type)
            }
            OP_ASHR => {
              if TRACE {println!("ashr.{} @{} @{} @{}",val_type_name(val_type),dst,src1,src2);}
              op_ashr(
                stack_get(&val_stack,src1),
                stack_get(&val_stack,src2),
              val_type)
            }
            _ => {panic!("unknown binary operation: {}",bin_op)}
          };
          stack_set(res,&mut val_stack,dst)
        }
        0x58..=0x5f => { // unary-op[val-type:3] [un_op:4][dst:4][src:4]
          let val_type = op_type & 0x7; // i8 i16 i32 i64 . f16 f32 f64
          let un_op = op_data & 0xf;
          let op_data = op_data >> 4;
          let dst = (op_data & 0xf) as usize;
          let op_data = op_data >> 4;
          let src = (op_data & 0xf) as usize +1;
          let op_data = op_data >> 4;
          const OP_NEG: u32 = 0;
          const OP_NOT: u32 = 1;
          const OP_SHLI: u32 = 2;
          const OP_LSHRI: u32 = 3;
          const OP_ASHRI: u32 = 4;
          let res = match un_op {
            OP_NEG => {
              if TRACE {println!("neg.{} @{} @{}",val_type_name(val_type),dst,src);}
              op_sub(
                0,
                stack_get(&val_stack,src),
              val_type)
            }
            OP_NOT => {
              if TRACE {println!("not.{} @{} @{}",val_type_name(val_type),dst,src);}
              op_xor(
                stack_get(&val_stack,src),
                !0,
              val_type)
            }
            OP_SHLI => {
              if TRACE {println!("shli.{} @{} @{} ${}",val_type_name(val_type),dst,src,op_data);}
              op_shl(
                stack_get(&val_stack,src),
                op_data as u64,
              val_type)
            }
            OP_LSHRI => {
              if TRACE {println!("lshri.{} @{} @{} ${}",val_type_name(val_type),dst,src,op_data);}
              op_lshr(
                stack_get(&val_stack,src),
                op_data as u64,
              val_type)
            }
            OP_ASHRI => {
              if TRACE {println!("ashri.{} @{} @{} ${}",val_type_name(val_type),dst,src,op_data);}
              op_ashr(
                stack_get(&val_stack,src),
                op_data as u64,
              val_type)
            }
            _ => {panic!("unknown unary operation: {}",un_op)}
          };
          stack_set(res,&mut val_stack,dst)
        }
        0x60..=0x6f => { // cvt[signed:1][dst_type:3] [dst:4][src:4][src-type:3]
          let signed = (op_type & 0x8) != 0;
          let dst_type = op_type & 0x7;
          let dst = (op_data & 0xf) as usize;
          let op_data = op_data >> 4;
          let src = (op_data & 0xf) as usize +1;
          let op_data = op_data >> 4;
          let src_type = op_data & 0x7;
          if TRACE {
            if signed {
                println!("cvts.{}.{} @{} @{}",val_type_name(src_type),val_type_name(dst_type),dst,src);
            } else {
                println!("cvtu.{}.{} @{} @{}",val_type_name(src_type),val_type_name(dst_type),dst,src);
            }
          }
          let res = op_cvt(stack_get(&val_stack,src),src_type,signed,dst_type);
          stack_set(res,&mut val_stack,dst)
        }
        0x80 => { // drop [imm:24]
            let count = op_data as usize;
            if TRACE {println!("drop ${}",count);}
            if count > val_stack.len() { panic!("stack underflow");}
            let new_length = val_stack.len() - count;
            val_stack.truncate(new_length);
        }
        0x81 => { // removeAt [count:8][loc:16]
            let count = (op_data & 0xff) as usize;
            let loc = (op_data >> 8) as usize + 1;
            if TRACE {println!("remove @{} ${}",loc,count);}
            if loc > val_stack.len() || count > loc { panic!("stack underflow");}
            let start = val_stack.len() - loc;
            let end = start + count;
            val_stack.drain(start..end);
        }
        0x82 => { // remove2 [loc1:8][loc2:8]
            let loc1 = (op_data & 0xff) as usize + 1;
            let loc2 = ((op_data >> 8) & 0xff) as usize + 1;
            if TRACE {println!("remove2 @{} @{}",loc1,loc2);}
            if loc1 > val_stack.len() { panic!("stack underflow");}
            val_stack.remove(val_stack.len() - loc1);
            if loc2 > val_stack.len() { panic!("stack underflow");}
            val_stack.remove(val_stack.len() - loc2);
        }
        0x83 => { // remove3 [loc1:8][loc2:8][loc3:8]
            let loc1 = (op_data & 0xff) as usize + 1;
            let op_data = op_data >> 8;
            let loc2 = (op_data & 0xff) as usize + 1;
            let loc3 = ((op_data >> 8) & 0xff) as usize + 1;
            if TRACE {println!("remove3 @{} @{} @{}",loc1,loc2,loc3);}
            if loc1 > val_stack.len() { panic!("stack underflow");}
            val_stack.remove(val_stack.len() - loc1);
            if loc2 > val_stack.len() { panic!("stack underflow");}
            val_stack.remove(val_stack.len() - loc2);
            if loc3 > val_stack.len() { panic!("stack underflow");}
            val_stack.remove(val_stack.len() - loc3);
        }
        0x84 => { // insert [src:4][dst:20]
            let src = (op_data & 0xf) as usize + 1;
            let dst = (op_data >> 4) as usize + 1;
            if TRACE {println!("insert @{} @{}",dst,src);}
            let val = stack_get(&val_stack,src);
            if dst > val_stack.len() { panic!("stack underflow");}
            val_stack.insert(dst,val);
        }
        0x85 => { // extract [dst:4][src:20]
            let dst = (op_data & 0xf) as usize;
            let src = (op_data >> 4) as usize;
            if TRACE {println!("extract @{} @{}",dst,src);}
            if src > val_stack.len() { panic!("stack underflow");}
            let val = val_stack.remove(src);
            stack_set(val,&mut val_stack,dst);
        }
        0x87 => { // copy-from [dst:4][src:20]
            let dst = (op_data & 0xf) as usize;
            let src = (op_data >> 4) as usize + 1;
            if TRACE {println!("copy @{} @{}",dst,src);}
            let val = stack_get(&val_stack,src);
            stack_set(val,&mut val_stack,dst);
        }
        0x88 => { // copy-to [src:4][dst:20]
            let src = (op_data & 0xf) as usize + 1;
            let dst = (op_data >> 4) as usize;
            if TRACE {println!("copy @{} @{}",dst,src);}
            let val = stack_get(&val_stack,src);
            stack_set(val,&mut val_stack,dst);
        }
        0x89 => { // copy [discard:4][dst:10][src:10]
            let to_drop = (op_data & 0xf) as usize;
            let op_data = op_data >> 4;
            let dst = (op_data & 0x3ff) as usize;
            let src = (op_data >> 10) as usize + 1;
            if TRACE {println!("copy.drop{} @{} @{}",to_drop,dst,src);}
            let val = stack_get(&val_stack,src);
            if to_drop > val_stack.len() { panic!("stack underflow");}
            let new_length = val_stack.len() - to_drop;
            val_stack.truncate(new_length);
            stack_set(val,&mut val_stack,dst);
        }
        0x8a => { // copy2 [discard:4][dst1:4][dst2:4][src1:4][src2:4]
            let to_drop = (op_data & 0xf) as usize;
            let op_data = op_data >> 4;
            let dst1 = (op_data & 0xf) as usize;
            let op_data = op_data >> 4;
            let dst2 = (op_data & 0xf) as usize;
            let op_data = op_data >> 4;
            let src1 = (op_data & 0xf) as usize + 1;
            let src2 = (op_data >> 4) as usize + 1;
            if TRACE {println!("copy2.drop{} @{} @{} @{} @{}",to_drop,dst1,src1,dst2,src2);}
            let val1 = stack_get(&val_stack,src1);
            let val2 = stack_get(&val_stack,src2);
            if to_drop > val_stack.len() { panic!("stack underflow");}
            let new_length = val_stack.len() - to_drop;
            val_stack.truncate(new_length);
            stack_set(val1,&mut val_stack,dst1);
            stack_set(val2,&mut val_stack,dst2);
        }
        0x8b => { // copy3 [dst1:4][dst2:4][dst3:4][src1:4][src2:4][src3:4]
            let dst1 = (op_data & 0xf) as usize;
            let op_data = op_data >> 4;
            let dst2 = (op_data & 0xf) as usize;
            let op_data = op_data >> 4;
            let dst3 = (op_data & 0xf) as usize;
            let op_data = op_data >> 4;
            let src1 = (op_data & 0xf) as usize +1;
            let op_data = op_data >> 4;
            let src2 = (op_data & 0xf) as usize +1;
            let src3 = (op_data >> 4) as usize +1;
            if TRACE {println!("copy3 @{} @{} @{} @{} @{} @{}",dst1,src1,dst2,src2,dst3,src3);}
            let val1 = stack_get(&val_stack,src1);
            let val2 = stack_get(&val_stack,src2);
            let val3 = stack_get(&val_stack,src3);
            stack_set(val1,&mut val_stack,dst1);
            stack_set(val2,&mut val_stack,dst2);
            stack_set(val3,&mut val_stack,dst3);
        }
        0x8c => { // swap [A:12][B:12]
            let a1 = (op_data & 0xfff) as usize + 1;
            let b1 = ((op_data >> 12) & 0xfff) as usize +1;
            if TRACE {println!("swap @{} @{}",a1,b1);}
            let val1 = stack_get(&val_stack,a1);
            let val2 = stack_get(&val_stack,b1);
            stack_set(val1,&mut val_stack,b1);
            stack_set(val2,&mut val_stack,a1);
        }
        0x8d => { // deep-swap [A:4][B:20]
            let a1 = (op_data & 0xf) as usize + 1;
            let b1 = (op_data >> 4) as usize + 1;
            if TRACE {println!("swap @{} @{}",a1,b1);}
            let val1 = stack_get(&val_stack,a1);
            let val2 = stack_get(&val_stack,b1);
            stack_set(val1,&mut val_stack,b1);
            stack_set(val2,&mut val_stack,a1);
        }
        0x8e => { // swap2 [a1:6][a2:6][b1:6][b2:6]
            let a1 = (op_data & 0x3f) as usize + 1;
            let op_data = op_data >> 6;
            let a2 = (op_data & 0x3f) as usize + 1;
            let op_data = op_data >> 6;
            let b1 = (op_data & 0x3f) as usize + 1;
            let b2 = (op_data >> 6) as usize + 1;
            if TRACE {println!("swap2 @{} @{} @{} @{}",a1,b1,a2,b2);}
            let val1 = stack_get(&val_stack,a1);
            let val2 = stack_get(&val_stack,a2);
            let val3 = stack_get(&val_stack,b1);
            let val4 = stack_get(&val_stack,b2);
            stack_set(val1,&mut val_stack,b1);
            stack_set(val2,&mut val_stack,b2);
            stack_set(val3,&mut val_stack,a1);
            stack_set(val4,&mut val_stack,a2);
        }
        0x8f => { // swap3 [A1:4][B1:4][C1:4][A2:4][B2:4][C2:4]
            let a1 = (op_data & 0xf) as usize + 1;
            let op_data = op_data >> 4;
            let a2 = (op_data & 0xf) as usize + 1;
            let op_data = op_data >> 4;
            let a3 = (op_data & 0xf) as usize + 1;
            let op_data = op_data >> 4;
            let b1 = (op_data & 0xf) as usize + 1;
            let op_data = op_data >> 4;
            let b2 = (op_data & 0xf) as usize + 1;
            let b3 = (op_data >> 4) as usize + 1;
            if TRACE {println!("swap3 @{} @{} @{} @{} @{} @{}",a1,b1,a2,b2,a3,b3);}
            let val1 = stack_get(&val_stack,a1);
            let val2 = stack_get(&val_stack,a2);
            let val3 = stack_get(&val_stack,a3);
            let val4 = stack_get(&val_stack,b1);
            let val5 = stack_get(&val_stack,b2);
            let val6 = stack_get(&val_stack,b3);
            stack_set(val1,&mut val_stack,b1);
            stack_set(val2,&mut val_stack,b2);
            stack_set(val3,&mut val_stack,b3);
            stack_set(val4,&mut val_stack,a1);
            stack_set(val5,&mut val_stack,a2);
            stack_set(val6,&mut val_stack,a3);
        }
        0x90 => { // local-alloc [count:24s]
          let count = ((op as i32) >> base_shift) as i64;
          if count > 0 {
            if TRACE {println!("alloc ${}",count);}
            rbp = (rbp as i64 - (count+7)&-8) as u64;
          } else {
            if TRACE {println!("dealloc ${}",-count);}
            rbp = (rbp as i64 - (count-7)&-8) as u64;
          }
        }
        0xff => { // syscall [call-id:24u]
          match op_data {
            0 => { // exit
              let res = val_stack.pop().unwrap();
              if TRACE {println!("syscall.exit");}
              process::exit(res as i32)
            }
            // 1 -> read
            2 => { // write
              let fd = val_stack.pop().unwrap();
              let count = val_stack.pop().unwrap();
              let ptr = val_stack.pop().unwrap();
              if TRACE {println!("syscall.write");}
              if fd != 1 {panic!("currently only write to stdout is supported")}
              let mut buf: Box<[u8]> = unsafe{ Box::<[u8]>::new_uninit_slice(count as usize).assume_init() };
              read_data(&program.allocations,ptr,&mut buf);
              let res = io::stdout().write_all(&buf);
              val_stack.push(if res.is_ok() {0} else {1});
            }
            _ => panic!("unknown syscall-id {}",op_data),
          }
        }
        _ => panic!("unknown op-code 0x{:x}",op_type),
      }
    }
    if TRACE {println!("{:09}: EOF: {:?}",ip,val_stack)}
}

fn load_file(file: &mut File) -> Option<Program> {
  let mut header_buf: [u64; 10] = [0; 10]; // [version][ip][code-addr][code-size][ro-addr][ro-data-size][rw-addr][rw-data-size][sp][stack-size]
  file.read_exact(u64_as_bytes_mut(&mut header_buf)).ok()?;
  let _version = header_buf[0];
  // sizes are given in chunks of 8bytes
  let ip = header_buf[1];
  let _code_addr = header_buf[2]; // memory-address of code, currently unused
  let code_size = (2*header_buf[3]) as usize; // convert u64 -> u32
  let ro_addr = header_buf[4]; // memory-address of ro-data, currently unused
  let ro_data_size = header_buf[5] as usize; // in u64
  let rw_addr = header_buf[6]; // memory-address of rw-data, currently unused
  let rw_data_size = header_buf[7]*8; // convert u64 -> byte
  let sp = header_buf[8];
  let stack_size = header_buf[9] as usize;
  let mut code_buffer = unsafe{ Box::<[u32]>::new_uninit_slice(code_size).assume_init() };
  file.read_exact(u32_as_bytes_mut(&mut code_buffer)).ok()?;
  let mut ro_buffer = unsafe{ Box::<[u64]>::new_uninit_slice(ro_data_size).assume_init() };
  file.read_exact(u64_as_bytes_mut(&mut ro_buffer)).ok()?;
  let mut rw_buffer = Vec::new();
  file.take(rw_data_size).read_to_end(&mut rw_buffer).ok()?;
  rw_buffer.resize(rw_data_size as usize,0);
  let mut allocations = AllocationTree::new();
  new_fixed_allocation(&mut allocations,&Allocation{
    start: ro_addr, data: u64_as_bytes_box(ro_buffer),
    readable: true, writeable: false, executable:false,
    allocation_type: AllocationType::STATIC,
  });
  new_fixed_allocation(&mut allocations,&Allocation{
    start: rw_addr, data: rw_buffer.into_boxed_slice(),
    readable: true, writeable: true, executable:false,
    allocation_type: AllocationType::STATIC,
  });
  let stack_data = u64_as_bytes_box(vec![0; stack_size].into_boxed_slice());
  new_fixed_allocation(&mut allocations,&Allocation{
    start: sp-((stack_size as u64)*8), data: stack_data,
    readable: true, writeable: true, executable:false,
    allocation_type: AllocationType::STATIC,
  });
  return Some(Program{code: code_buffer,ip: ip,sp: sp,ro_addr:ro_addr,rw_addr:rw_addr,allocations: allocations})
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
