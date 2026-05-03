#!/usr/bin/python

class StackLocation:
  def __init__(self,index):
    self.index = index
  def __repr__(self):
    return f"StackLocation({self.index})"

class Label:
  def __init__(self,name):
    self.name = name
  def __repr__(self):
    return f"Label({self.name})"

class OpLoadi:
  def __init__(self,dst,value,*,shift):
    self.value = value
    self.dst = dst
    self.shift = shift
  def __repr__(self):
    return f"OpLoadi(dst={self.dst},value={self.value},shift={self.shift})"

class OpBinary:
  def __init__(self,base_op,dst,src1,src2,*,cmp_type,val_type):
    self.base_op = base_op
    self.val_type = val_type
    self.cmp_type = cmp_type
    self.dst = dst
    self.src1 = src1
    self.src2 = src2
  def __repr__(self):
    return f"OpBinary({self.base_op},dst={self.dst},src1={self.src1},src2={self.src2},cmp_type={self.cmp_type},val_type={self.val_type})"

class OpJmp:
  def __init__(self,jmp_type,target):
    self.jmp_type = jmp_type
    self.target = target
  def __repr__(self):
    return f"OpJmp({self.jmp_type},target={self.target})"

class OpJmpIf:
  def __init__(self,jmp_type,arg,target):
    self.jmp_type = jmp_type
    self.arg = arg
    self.target = target
  def __repr__(self):
    return f"OpJmpIf({self.jmp_type},arg={self.arg},target={self.target})"

class OpRet:
  def __init__(self):
    pass
  def __repr__(self):
    return f"OpRet()"

class OpCopy:
  def __init__(self,dst,src,*,drop_count):
    self.dst = dst
    self.src = src
    self.drop_count = drop_count
  def __repr__(self):
    return f"OpCopy(dst={self.dst},src={self.src},drop_count={self.drop_count})"

class OpSwap:
  def __init__(self,loc1,loc2):
    self.loc1 = loc1
    self.loc2 = loc2
  def __repr__(self):
    return f"OpSwap(loc1={self.loc1},loc2={self.loc2})"

# TODO: assembly/disassembly of files
def parseLoc(val):
  if val[0] != '@':
    raise Exception("location has to start with @ got: "+val)
  return StackLocation(int(val[1:]))

def parseInt(val):
  if val[0] != '$':
    raise Exception("integer literal has to start with $ got: "+val)
  if val[1] == 'x': ## hex literal
    return int(val[2:],16)
  return int(val[1:])

def parseArg(val):
  if val[0] == '@':
    return parseLoc(val)
  if val[0] == ':':
    return Label(val[1:])
  return parseInt(val)

def parseCmpType(cmp_type):
  cmp_type = cmp_type.lower()
  if cmp_type in ["eq","ne","lt","le","ult","ule"]:
    return (cmp_type,False)
  elif cmp_type in ["gt","ge","ugt","uge"]:
    cmp_type[-2]='l'
    return (cmp_type,True)
  raise Exception("unsupported cmp_type: "+cmp_type)

def parseValType(val_type):
  val_type = val_type.lower()
  if val_type in ["i8","i16","i32","i64","f16","f32","f64"]:
    return val_type
  raise Exception("unsupported val_type: "+val_type)

def parseLine(line):
  line = line.strip()
  hash_pos = line.find('#')
  if hash_pos != -1:
    line = line[:hash_pos]
  parts = line.split()
  if len(parts) < 1:
    return []
  op_code = parts[0].lower()
  args = parts[1:]
  if op_code == "loadi":
    dst = parseLoc(args[0])
    arg = parseInt(args[1])
    return [OpLoadi(dst,arg, shift = 0)]
  elif op_code.startswith("loadi."):
    dst = parseLoc(args[0])
    arg = parseInt(args[1])
    shift = int(op_code[len("loadi."):])
    return [OpLoadi(dst,arg, shift = shift)]
  elif (op_code.startswith("cmp.") or
     op_code.startswith("add.") or op_code.startswith("sub.") or op_code.startswith("mul.") or
     op_code.startswith("and.") or op_code.startswith("or.") or op_code.startswith("xor.") or
     op_code.startswith("shl.") or op_code.startswith("lshr.") or op_code.startswith("ashr.")
    ):
    base_op, val_type = op_code.split('.')
    val_type = parseValType(val_type)
    need_swap = False
    if base_op == "cmp":
      cmp_type, need_swap = parseCmpType(args[0])
      args = args[1:]
    else:
      cmp_type = None
    dst = parseLoc(args[0])
    src1 = parseArg(args[1])
    src2 = parseArg(args[2])
    if need_swap:
      src1, src2 = src2, src1
    return [OpBinary(base_op,dst,src1,src2, cmp_type = cmp_type,val_type = val_type)]
  elif op_code == "jmp" or op_code == "call" or op_code == "jmp.abs" or op_code == "call.abs":
    target = parseArg(args[0])
    return [OpJmp(op_code, target)]
  elif op_code == "ret":
    return [OpRet()]
  elif op_code == "jz" or op_code == "jnz":
    if args[0] == "pop":
      op_code = op_code + " pop"
      arg = None
    else:
      arg = parseLoc(args[0])
    target = parseArg(args[1])
    return [OpJmpIf(op_code, arg, target)]
  elif op_code == "copy":
    loc1 = parseLoc(args[0])
    loc2 = parseLoc(args[1])
    return [OpCopy(loc1, loc2, drop_count = 0)]
  elif op_code.startswith("copy.drop"):
    drop_count = int(op_code[len("copy.drop"):])
    loc1 = parseLoc(args[0])
    loc2 = parseLoc(args[1])
    return [OpCopy(loc1, loc2, drop_count = drop_count)]
  elif op_code == "swap":
    loc1 = parseLoc(args[0])
    loc2 = parseLoc(args[1])
    return [OpSwap(loc1, loc2)]
  raise Exception("unknown op_code: "+op_code)

def parse(code):
  return [op for ops in [parseLine(line)for line in code.split('\n')] for op in ops]

def parseFile(srcFile="src.txt"):
  with open(srcFile,mode="r") as f:
    return parse(f.read())

def writeU32(f,val):
    f.write(bytes([(val >> 8*s) & 0xFF for s in range(4)]))
    return 4;

def writeU64(f,val):
    f.write(bytes([(val >> 8*s) & 0xFF for s in range(8)]))
    return 8;

def writeU32s(f,vals):
    f.write(bytes([(val >> 8*s) & 0xFF for val in vals for s in range(4)]))
    return 4*len(vals);

def generate(out="in.cctbc"):
    ops = [
0x00000000, # loadi dst:0 val:0
0x00001000, # loadi dst:0 val:1
0x000c2033, # cmpi.i64 lt dst:0 swap src:3 val:0
0x00000427, # jz.drop dst:4
0x00101153, # add.i64 dst:1 src1:1 src2:2
0xffff233b, # addi.i64 dst:3 src:3 val:-1
0x0001008c, # swap arg1:1 arg2:2
0xfffffa22, # jmp dst:-6
0x00401289, # copy drop: 2 dst:1 src:2
0xffffff23, # ret
0x00005000, # loadi dst:0 val:5
0x00000021, # call_abs val:0
    ]
    start = 10
    print([hex(op)for op in ops])
    ## file-format
    ## [version][ip][code-addr][code-size][ro-addr][ro-data-size][rw-addr][rw-data-size][sp][stack-size]
    stack_pointer = 0x1_0000_0000
    stack_size = 0x10_0000
    with open(out,mode="wb") as f:
        writeU64(f,0) ## reserved
        writeU64(f,start)
        writeU64(f,0) ## code-addr
        writeU64(f,(len(ops)+1)//2)
        writeU64(f,0) ## ro-data-addr
        writeU64(f,0) ## ro-data-size
        writeU64(f,0) ## rw-data-addr
        writeU64(f,0) ## rw-data-addr
        writeU64(f,stack_pointer) ## sp
        writeU64(f,stack_size) ## stack-size
        writeU32s(f,ops) ## code
        if len(ops) & 1: ## padding
          writeU32(f,0)

generate()
