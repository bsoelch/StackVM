#!/usr/bin/python

class StackLocation:
  def __init__(self,index):
    self.index = index
  def __repr__(self):
    return f"StackLocation({self.index})"

class RelativeAddress:
  def __init__(self,base,offset):
    self.base = base
    self.offset = offset
  def __repr__(self):
    return f"RelativeAddress({self.base},{self.offset})"

class Label:
  def __init__(self,name,*,offset = 0,base = None):
    self.name = name
    self.offset = offset
    self.base = base
  def __repr__(self):
    return f"Label({self.name},offset={self.offset},base={self.base})"

def expectStackLocation(val,minIndex,maxIndex):
  if type(val) != StackLocation:
    raise Exception(f"expected stack location got: {str(type(val))} {repr(val)}")
  if val.index < minIndex or val.index > maxIndex:
    raise Exception(f"stack index {val.index} outside allowed range {minIndex} to {maxIndex}")
  return val

def valTypeId(name):
  return ["i8","i16","i32","i64","","f16","f32","f64"].index(name)
def cmpTypeId(name):
  return ["eq","ne","","","lt","le","ult","ule"].index(name)
def jumpTypeId(name):
  return ["jmp.abs","call.abs","jmp","call","jnz","jz","jnz pop","jz pop"].index(name)
def binOpId(name):
  return ["cmp","add","sub","mul","","","","","and","or","xor","shl","lshr","ashr"].index(name)
def unaryOpId(name):
  return ["neg","not","shl","lshr","ashr"].index(name)
def binImmOpId(name):
  return {"add":0x38,"mul":0x40,"and":0x48,"or":0x4c}[name]
def encodeSize(size):
  return [1,2,4,8].index(size)
def loadBaseId(name,is_store):
  return {("bp",False):0x12,("bp",True):0x13,("ip",False):0x14,("ro_data",False):0x15,("rw_data",False):0x16,("rw_data",True):0x17}[(name,is_store)]
LOAD2_FLAG = 0x8
def addrBaseId(name):
  return {"bp":0x0c,"ip":0x0d,"ro_data":0x0e,"rw_data":0x0f}[name]

class OpLoadi:
  def __init__(self,dst,value,*,shift):
    self.value = value
    self.dst = expectStackLocation(dst,0,15)
    self.shift = shift
  def __repr__(self):
    return f"OpLoadi(dst={self.dst},value={self.value},shift={self.shift})"
  def generate(self,prog):
    prog.appendU32(self.value << 12 | (self.dst.index & 0xf) << 8 | (self.shift & 0xf))

class OpLoad:
  def __init__(self,is_store,size,val,src,offset):
    self.is_store = is_store
    self.size = size
    self.val = expectStackLocation(val,0+self.is_store,15+self.is_store)
    self.src = expectStackLocation(src,1,15)
    self.offset = offset
  def __repr__(self):
    return f"OpLoad(is_store={self.is_store},size={self.size},val={self.val},offset={self.offset})"
  def generate(self,prog):
    prog.appendU32(self.offset << 18 | (encodeSize(self.size) << 16) | ((self.src.index-1) & 0xf) << 12 | ((self.val.index-self.is_store) & 0xf) << 8 | (0x11 if self.is_store else 0x10))

class OpLoadRelative:
  def __init__(self,base,is_store,size,val,offset):
    self.base = base
    self.is_store = is_store
    self.size = size
    self.val = expectStackLocation(val,0+self.is_store,15+self.is_store)
    self.offset = offset
  def __repr__(self):
    return f"OpLoadRelative(base={self.base},is_store={self.is_store},size={self.size},val={self.val},offset={self.offset})"
  def generate(self,prog):
    prog.appendU32(self.offset << 14 | (encodeSize(self.size) << 12) | ((self.val.index-self.is_store) & 0xf) << 8 | loadBaseId(self.base,self.is_store))

class OpLoadLabel:
  def __init__(self,base,is_store,size,val,offset):
    self.is_store = is_store
    self.size = size
    self.val = expectStackLocation(val,0+self.is_store,15+self.is_store)
    self.base = base
    self.offset = offset
  def __repr__(self):
    return f"OpLoadLabel(base={self.base},is_store={self.is_store},size={self.size},val={self.val},offset={self.offset})"

class OpLoad2:
  def __init__(self,is_store,val1,val2,src,offset):
    self.is_store = is_store
    self.val1 = expectStackLocation(val1,0+self.is_store,15+self.is_store)
    self.val2 = expectStackLocation(val2,0+self.is_store,15+self.is_store)
    self.src = expectStackLocation(src,1,15)
    self.offset = offset
  def __repr__(self):
    return f"OpLoad2(is_store={self.is_store},val1={self.val1},val2={self.val2},offset={self.offset})"
  def generate(self,prog):
    prog.appendU32(self.offset << 20 | ((self.src.index-1) & 0xf) << 16 | ((self.val2.index-self.is_store) & 0xf) << 12 | ((self.val1.index-self.is_store) & 0xf) << 8 | (0x11 if self.is_store else 0x10) | LOAD2_FLAG)

class OpLoad2Relative:
  def __init__(self,base,is_store,val1,val2,offset):
    self.base = base
    self.is_store = is_store
    self.val1 = expectStackLocation(val1,0+self.is_store,15+self.is_store)
    self.val2 = expectStackLocation(val2,0+self.is_store,15+self.is_store)
    self.offset = offset
  def __repr__(self):
    return f"OpLoad2Relative(base={self.base},is_store={self.is_store},val1={self.val1},val2={self.val2},offset={self.offset})"
  def generate(self,prog):
    prog.appendU32(self.offset << 16 | ((self.val2.index-self.is_store) & 0xf) << 12 | ((self.val1.index-self.is_store) & 0xf) << 8 | loadBaseId(self.base,self.is_store) | LOAD2_FLAG)

class OpLoad2Label:
  def __init__(self,base,is_store,val1,val2,offset):
    self.is_store = is_store
    self.val1 = expectStackLocation(val1,0+self.is_store,15+self.is_store)
    self.val2 = expectStackLocation(val2,0+self.is_store,15+self.is_store)
    self.base = base
    self.offset = offset
  def __repr__(self):
    return f"OpLoad2Label(base={self.base},is_store={self.is_store},val1={self.val1},val2={self.val2},offset={self.offset})"

class OpAddr:
  def __init__(self,base,dst,offset):
    self.base = base
    self.dst = expectStackLocation(dst,0,15)
    self.offset = offset
  def __repr__(self):
    return f"OpAddr(base={self.base},dst={self.dst},offset={self.offset})"
  def generate(self,prog):
    prog.appendU32(self.offset << 12 | (self.dst.index & 0xf) << 8 | addrBaseId(self.base))

class OpBinary:
  def __init__(self,base_op,dst,src1,src2,*,val_type,cmp_type):
    self.base_op = base_op
    self.val_type = val_type
    self.cmp_type = cmp_type
    self.dst = expectStackLocation(dst,0,15)
    self.src1 = expectStackLocation(src1,1,16)
    self.src2 = expectStackLocation(src2,1,16)
  def __repr__(self):
    return f"OpBinary({self.base_op},dst={self.dst},src1={self.src1},src2={self.src2},val_type={self.val_type},cmp_type={self.cmp_type})"
  def generate(self,prog):
    prog.appendU32((cmpTypeId(self.cmp_type) << 24 if self.cmp_type else 0) | ((self.src2.index-1) & 0xf) << 20 | ((self.src1.index-1) & 0xf) << 16 | (self.dst.index & 0xf) << 12 | binOpId(self.base_op) << 8 | (valTypeId(self.val_type) | 0x50))

class OpCmpImm:
  def __init__(self,dst,src1,src2,*,val_type,cmp_type,swap_args):
    self.val_type = val_type
    self.cmp_type = cmp_type
    self.swap_args = swap_args
    self.dst = expectStackLocation(dst,0,15)
    self.src1 = expectStackLocation(src1,1,16)
    self.src2 = src2
  def __repr__(self):
    return f"OpCmpImm(dst={self.dst},src1={self.src1},src2={self.src2},val_type={self.val_type},cmp_type={self.cmp_type},swap_args={self.swap_args})"
  def generate(self,prog):
    if self.val_type[0] == "f": raise Exception("float constants are not supported")
    if type(self.src2) != int: raise Exception("unsupported constant type: "+type(self.src2))
    prog.appendU32(self.src2 << 20 | ((0x8 if self.swap_args else 0) | cmpTypeId(self.cmp_type) ) << 16 | ((self.src1.index-1) & 0xf) << 12 | (self.dst.index & 0xf) << 8 | (valTypeId(self.val_type) | 0x30))

class OpBinaryImm:
  def __init__(self,base_op,dst,src1,src2,*,val_type):
    self.base_op = base_op
    self.val_type = val_type
    self.dst = expectStackLocation(dst,0,15)
    self.src1 = expectStackLocation(src1,1,16)
    self.src2 = src2
  def __repr__(self):
    return f"OpBinaryImm({self.base_op},dst={self.dst},src1={self.src1},src2={self.src2},val_type={self.val_type})"
  def generate(self,prog):
    if self.val_type[0] == "f": raise Exception("float constants are not supported")
    if type(self.src2) != int: raise Exception("unsupported constant type: "+type(self.src2))
    prog.appendU32(self.src2 << 16 | ((self.src1.index-1) & 0xf) << 12 | (self.dst.index & 0xf) << 8 | (valTypeId(self.val_type) | binImmOpId(self.base_op)))

class OpShiftImm:
  def __init__(self,base_op,dst,src1,src2,*,val_type):
    self.base_op = base_op
    self.val_type = val_type
    self.dst = expectStackLocation(dst,0,15)
    self.src1 = expectStackLocation(src1,1,16)
    self.src2 = src2
  def __repr__(self):
    return f"OpShiftImm({self.base_op},dst={self.dst},src1={self.src1},src2={self.src2},val_type={self.val_type})"
  def generate(self,prog):
    if self.val_type[0] == "f": raise Exception("float constants are not supported")
    if type(self.src2) != int: raise Exception("unsupported constant type: "+type(self.src2))
    prog.appendU32(self.src2 << 20 | ((self.src1.index-1) & 0xf) << 16 | (self.dst.index & 0xf) << 12 | unaryOpId(base_op) << 8 | (valTypeId(self.val_type) | 0x58))

class OpUnary:
  def __init__(self,base_op,dst,src,*,val_type):
    self.base_op = base_op
    self.val_type = val_type
    self.dst = expectStackLocation(dst,0,15)
    self.src = expectStackLocation(src,1,16)
  def __repr__(self):
    return f"OpUnary({self.base_op},dst={self.dst},src={self.src},val_type={self.val_type})"
  def generate(self,prog):
    prog.appendU32(((self.src.index-1) & 0xf) << 16 | (self.dst.index & 0xf) << 12 | unaryOpId(self.base_op) << 8 | (valTypeId(self.val_type) | 0x58))

class OpCvt:
  def __init__(self,dst,src,*,src_type,signed,dst_type):
    self.src_type = src_type
    self.dst_type = dst_type
    self.signed = signed
    self.dst = expectStackLocation(dst,0,15)
    self.src = expectStackLocation(src,1,16)
  def __repr__(self):
    return f"OpCvt(signed={self.signed},dst={self.dst},src={self.src},src_type={self.src_type},dst_type={self.dst_type})"
  def generate(self,prog):
    prog.appendU32(valTypeId(self.src_type) << 16 | ((self.src.index-1) & 0xf) << 12 | (self.dst.index & 0xf) << 8 | (valTypeId(self.dst_type) | (0x68 if self.signed else 0x60)))

class OpJmp:
  def __init__(self,jmp_type,target,*,is_long_jump=False):
    self.jmp_type = jmp_type
    self.is_long_jump = is_long_jump
    self.target = target
  def __repr__(self):
    return f"OpJmp({self.jmp_type},target={self.target})"
  def generate(self,prog):
    prog.appendU32(self.target << 8 | ((0x8 if self.is_long_jump else 0) | jumpTypeId(self.jmp_type) | 0x20))

class OpJmpIf:
  def __init__(self,jmp_type,arg,target,*,is_long_jump=False):
    self.jmp_type = jmp_type
    self.is_long_jump = is_long_jump
    self.arg = expectStackLocation(arg,1,16)
    self.target = target
  def __repr__(self):
    return f"OpJmpIf({self.jmp_type},arg={self.arg},target={self.target})"
  def generate(self,prog):
    prog.appendU32(self.target << 12 | ((self.arg.index-1)&0xf) | ((0x8 if self.is_long_jump else 0) | jumpTypeId(self.jmp_type) | 0x20))

class OpRet:
  def __init__(self):
    pass
  def __repr__(self):
    return f"OpRet()"
  def generate(self,prog):
    prog.appendU32(0xffffff23)

# TODO: support variants copyFrom/To, deepSwap
class OpCopy:
  def __init__(self,dst,src,*,drop_count):
    self.dst = expectStackLocation(dst,0,1023)
    self.src = expectStackLocation(src,1,1024)
    self.drop_count = drop_count
  def __repr__(self):
    return f"OpCopy(dst={self.dst},src={self.src},drop_count={self.drop_count})"
  def generate(self,prog):
    prog.appendU32(((self.src.index-1)&0xf) << 22 | (self.dst.index&0xf) << 12 | (self.drop_count << 8) | 0x89)

class OpSwap:
  def __init__(self,loc1,loc2):
    self.loc1 = expectStackLocation(loc1,1,4096)
    self.loc2 = expectStackLocation(loc2,1,4096)
  def __repr__(self):
    return f"OpSwap(loc1={self.loc1},loc2={self.loc2})"
  def generate(self,prog):
    prog.appendU32(((self.loc2.index-1)&0xf) << 20 | ((self.loc1.index-1)&0xf) << 8 | 0x8c)

class OpAlloc:
  def __init__(self,count):
    self.count = count
  def __repr__(self):
    return f"OpAlloc(count={self.count})"
  def generate(self,prog):
    prog.appendU32(self.count << 8 | 0x90)

class OpLabel:
  def __init__(self,name):
    self.name = name
  def __repr__(self):
    return f"OpLabel(name={self.name})"
  def generate(self,prog):
    pass

class OpData:
  def __init__(self,val_type,data,*,has_label):
    self.val_type = val_type
    self.has_label_arg = has_label
    self.data = data
  def __repr__(self):
    return f"OpData(val_type={self.val_type},data={self.data})"
  def generate(self,prog):
    if self.val_type == "i8":
      prog.appendBytes(self.data)
      return
    if self.val_type == "i64":
      prog.appendU64s(self.data)
      return
    if self.val_type == "i32":
      prog.appendU32s(self.data)
      return
    if self.val_type == "i16":
      prog.appendU16s(self.data)
      return
    raise Exception(f"encoding {self.val_type} data is not yet supported")

class OpAlign:
  def __init__(self,byte_alignment):
    self.byte_alignment = byte_alignment
  def __repr__(self):
    return f"OpAlign(byte_alignment={self.byte_alignment})"
  def generate(self,prog):
    prog.align(self.byte_alignment)

class OpStart:
  def __init__(self):
    pass
  def __repr__(self):
    return f"OpStart()"
  def generate(self,prog):
    prog.start = len(prog.code)

class Program:
  def __init__(self):
    self.code = []
    self.start = 0
    self.ro_data = []
    self.rw_data = []
    self.section = "code"
  def appendU32(self,val):
    if self.section == "code":
      self.code.append(val)
    elif self.section == "ro_data":
      self.ro_data.extend(val.to_bytes(4, byteorder="little", signed=False))
    elif self.section == "rw_data":
      self.rw_data.extend(val.to_bytes(4, byteorder="little", signed=False))
  def appendU32s(self,vals):
    if self.section == "code":
      self.code.extend(vals)
    else:
      for val in vals:self.appendU32(val)
  def appendU64s(self,vals):
    self.appendBytes((b for val in vals for b in val.to_bytes(8, byteorder="little", signed=False)))
  def appendU16s(self,vals):
    self.appendBytes((b for val in vals for b in val.to_bytes(2, byteorder="little", signed=False)))
  def appendBytes(self,vals):
    if self.section == "code":
      self.code.extend([int.from_bytes(vals[4*i:4*i+4],byteorder="little",signed=False)for i in range((len(vals)+3)//4)])
    elif self.section == "ro_data":
      self.ro_data.extend(vals)
    elif self.section == "rw_data":
      self.rw_data.extend(vals)
  def align(self,byte_alignment):
    if byte_alignment < 2: return
    if self.section == "code":
      if byte_alignment < 4: return
      byte_alignment//=4
      self.code.extend([0]*(byte_alignment-len(self.code)%byte_alignment))
    elif self.section == "ro_data":
      self.ro_data.extend([0]*(byte_alignment-len(self.ro_data)%byte_alignment))
    elif self.section == "rw_data":
      self.rw_data.extend([0]*(byte_alignment-len(self.rw_data)%byte_alignment))

def parseLoc(val):
  if val[0] != '@':
    raise Exception("location has to start with @ got: "+val)
  return StackLocation(int(val[1:]))

def parseInt(val):
  if val[0] != '$':
    raise Exception("integer literal has to start with $ got: "+val)
  ## TODO? $d,$f,$h -> f64/f32/f16  , $b -> binary int
  if val[1] == 'x': ## hex literal
    return int(val[2:],16)
  return int(val[1:])

def parseLabel(val):
  if val[0] != ':':
    raise Exception("label has to start with : got: "+val)
  if "+" in val:
    base,offset = val.split("+")
    return Label(base[1:],offset = int(offset))
  if "-" in val:
    base,offset = val.split("-")
    if offset[0] == ':':
      return Label(base[1:],base = offset[1:])
    else:
      return Label(base[1:],offset = int(offset))
  return Label(val[1:])

def parseArg(val):
  ## offset-val: :label+offset, @<index>+offset, @ip+offset, @bp+offset
  if val[0] == '@':
    return parseLoc(val)
  if val[0] == ':':
    return parseLabel(val)
  return parseInt(val)

def parseAbsoluteAddress(val):
  if val[0] == ':':
    return parseLabel(val)
  if val.startswith("@ro_data"):
    if len(val) == 8: return RelativeAddress("ro_data",0)
    if val[8] not in "+-":raise Exception("offset has to start with + or -: "+val)
    return RelativeAddress("ro_data",int(val[8:]))
  if val.startswith("@rw_data"):
    if len(val) == 8: return RelativeAddress("rw_data",0)
    if val[8] not in "+-":raise Exception("offset has to start with + or -: "+val)
    return RelativeAddress("rw_data",int(val[8:]))
  return parseInt(val)

def parseAddress(val):
  if val.startswith("@ip"):
    if len(val) == 3: return RelativeAddress("ip",0)
    if val[3] not in "+-":raise Exception("offset has to start with + or -: "+val)
    return RelativeAddress("ip",int(val[3:]))
  if val.startswith("@bp"):
    if len(val) == 3: return RelativeAddress("bp",0)
    if val[3] not in "+-":raise Exception("offset has to start with + or -: "+val)
    return RelativeAddress("bp",int(val[3:]))
  if val[0] == '@':
    if "+" in val:
      base,offset = val.split("+")
      return RelativeAddress(StackLocation(int(base[1:])),int(offset))
    return parseLoc(val)
  return parseAbsoluteAddress(val)

def parseData(val,val_type):
  if val[0] == '"':
    if val_type != "i8": raise Exception("string-data is only supported for type i8")
    return [*bytes(val[1:],encoding="utf8")]
  if val[0] != '$':
    if val_type != "i64": raise Exception("label-data is only supported for type i64")
    return [parseAbsoluteAddress(val)]
  return [parseInt(val)]

def parseCmpType(cmp_type):
  cmp_type = cmp_type.lower()
  if cmp_type in ["eq","ne","lt","le","ult","ule"]:
    return (cmp_type,False)
  elif cmp_type in ["gt","ge","ugt","uge"]:
    cmp_type=cmp_type.replace('g','l')
    return (cmp_type,True)
  raise Exception("unsupported cmp_type: "+cmp_type)

def parseValType(val_type):
  val_type = val_type.lower()
  if val_type in ["i8","i16","i32","i64","f16","f32","f64"]:
    return val_type
  raise Exception("unsupported val_type: "+val_type)

LOADI_MIN_VAL = -0x80000
LOADI_MAX_VAL = 0x7ffff
LOADI_MASK = 0xfffff

def splitLine(line):
  i = 0
  while i < len(line) and not line[i].isspace(): i+=1
  op_code = line[:i]
  line = line[i:]
  args = []
  while len(line) > 0:
    i = 0
    while i < len(line) and line[i].isspace(): i+=1
    line = line[i:]
    if len(line) == 0:break
    if line[0] == '"':
      i = 1
      while i < len(line) and line[i] != '"': i+=1
      args.append(line[:i])
      line = line[i+1:]
      continue
    i = 0
    if line[0] in "+-":
      if len(args) == 0: raise Exception(f"{line[0]} cannot be the first argument")
      args[-1]+=line[0]
      i = 1
      while i < len(line) and line[i].isspace(): i+=1
      line = line[i:]
      while i < len(line) and not line[i].isspace(): i+=1
      args[-1]+=line[:i]
      line = line[i+1:]
      continue
    while i < len(line) and not line[i].isspace(): i+=1
    args.append(line[:i])
    line = line[i+1:]
  return op_code, args

class SourceFile:
  def __init__(self):
    self.has_label = False
    self.code = []
    self.ro_data = []
    self.rw_data = []
    self.section = "code"

  def appendOp(self,value):
    if self.section == "code":
      self.code.append(value)
    elif self.section == "ro_data":
      self.code.append(value)
    elif self.section == "rw_data":
      self.code.append(value)
    else:
      raise Exception("unsupported section: "+self.section)
    
  def parseLine(self,line):
    line = line.strip()
    hash_pos = line.find('#')
    if hash_pos != -1:
      line = line[:hash_pos]
    op_code, args = splitLine(line)
    if len(op_code) == 0:
      return
    if op_code == "loadi":
      dst = parseLoc(args[0])
      arg = parseInt(args[1])
      if arg > LOADI_MAX_VAL or arg < LOADI_MIN_VAL:
        raise Exception(f"argument of loadi has to be between {LOADI_MIN_VAL} and {LOADI_MAX_VAL}")
      ## TODO? automatically split value into multiple loadi's
      self.appendOp(OpLoadi(dst,arg, shift = 0))
    elif op_code.startswith("loadi."):
      dst = parseLoc(args[0])
      arg = parseInt(args[1])
      if arg > LOADI_MAX_VAL or arg < LOADI_MIN_VAL:
        raise Exception(f"argument of loadi has to be between {LOADI_MIN_VAL} and {LOADI_MAX_VAL}")
      shift = int(op_code[len("loadi."):])
      if shift < 0 or shift > 44:
        raise Exception(f"shift has to be between 0 and 44 got: {shift}")
      if (shift % 4) != 0:
        raise Exception(f"shift has to be divisible by 4 got: {shift}")
      shift //= 4
      self.appendOp(OpLoadi(dst,arg, shift = shift))
    elif op_code.startswith("load.") or op_code.startswith("store."):
      is_store = (op_code[0] == 's')
      size = int(op_code[(len("store.") if is_store else len("load.")):])
      if size not in [1,2,4,8]:
        raise Exception(f"size has to be one of 1,2,4,8 got: {size}")
      dst = parseLoc(args[0])
      addr = parseArg(args[1])
      ## TODO: check offset range
      if type(addr) == StackLocation:
        self.appendOp(OpLoad(is_store,size,dst,addr,0))
      elif type(addr) == RelativeAddress and type(addr.base) == StackLocation:
        self.appendOp(OpLoad(is_store,size,dst,addr.base,addr.offset))
      elif type(addr) == RelativeAddress:
        if is_store and addr.base in ["ip","ro_data"]: raise Exception(f"cannot store to {addr.base}-relative address")
        self.appendOp(OpLoadRelative(addr.base,is_store,size,dst,addr.offset))
      elif type(addr) == Label:
        self.has_label = True
        self.appendOp(OpLoadLabel(addr,is_store,dst,0))
      else:
        raise Exception(f"load/store is not implemented: {size} {dst} {addr}")
    elif op_code == "load2" or op_code == "store2":
      is_store = (op_code[0] == 's')
      dst1 = parseLoc(args[0])
      dst2 = parseLoc(args[1])
      addr = parseArg(args[2])
      if type(addr) == StackLocation:
        self.appendOp(OpLoad2(is_store,dst1,dst2,addr,0))
      elif type(addr) == RelativeAddress and type(addr.base) == StackLocation:
        self.appendOp(OpLoad2(is_store,dst1,dst2,addr.base,addr.offset))
      elif type(addr) == RelativeAddress:
        if is_store and addr.base in ["ip","ro_data"]: raise Exception(f"cannot store to {addr.base}-relative address")
        self.appendOp(OpLoad2Relative(addr.base,is_store,dst1,dst2,addr.offset))
      elif type(addr) == Label:
        self.has_label = True
        self.appendOp(OpLoad2Label(addr,is_store,dst1,dst2,0))
      else:
        raise Exception(f"load2/store2 is not implemented: {dst1} {dst2} {addr}")
    elif op_code == "addr":
      dst = parseLoc(args[0])
      addr = parseArg(args[1])
      if type(addr) == RelativeAddress and type(addr.base) != StackLocation:
        self.appendOp(OpAddr(addr.base,dst,addr.offset))
      else:
        raise Exception(f"addr is not implemented: {dst} {addr}")
    elif (op_code.startswith("cmp.") or op_code.startswith("cmpi.") or
       op_code.startswith("add.") or op_code.startswith("addi.") or
       op_code.startswith("sub.") or op_code.startswith("subi.") or
       op_code.startswith("mul.") or  op_code.startswith("muli.") or
       op_code.startswith("and.") or op_code.startswith("andi.") or
       op_code.startswith("or.") or op_code.startswith("ori.") or op_code.startswith("xor.") or
       op_code.startswith("shl.") or op_code.startswith("lshr.") or op_code.startswith("ashr.") or
       op_code.startswith("shli.") or op_code.startswith("lshri.") or op_code.startswith("ashri.")
      ):
      base_op, val_type = op_code.split('.')
      is_immediate = False
      if base_op[-1] == 'i':
        is_immediate = True
        base_op = base_op[:-1]
      val_type = parseValType(val_type)
      need_swap = False
      if base_op == "cmp":
        cmp_type, need_swap = parseCmpType(args[0])
        args = args[1:]
      else:
        cmp_type = None
      dst = parseLoc(args[0])
      src1 = parseArg(args[1])
      if type(src1) != StackLocation: is_immediate = True
      src2 = parseArg(args[2])
      if type(src2) != StackLocation: is_immediate = True
      if need_swap:
        src1, src2 = src2, src1
      if is_immediate:
        if base_op == "cmp":
          swap_args = False
          if type(src1) != StackLocation:
            if type(src2) != StackLocation:
              raise Exception(f"at least one operand of {base_op}i has to be StackLocation")
            src1, src2 = src2, src1
            swap_args = True
          self.appendOp(OpCmpImm(dst,src1,src2, cmp_type = cmp_type,val_type = val_type,swap_args = swap_args))
        elif base_op == "sub": ## sub val, imm -> add val, -imm
          if type(src1) != StackLocation:
            raise Exception(f"left operand of {base_op}i has to be StackLocation")
          if type(src2) != int and type(src) != float:
            raise Exception(f"unsupported operand for {base_op}i: "+src2)
          self.appendOp(OpBinaryImm("add",dst,src1,-src2, val_type = val_type))
        elif base_op in ["add","mul","and","or"]:
          if type(src1) != StackLocation:
            if type(src2) != StackLocation:
              raise Exception(f"at least one operand of {base_op}i has to be StackLocation")
            src1, src2 = src2, src1 ## operation is commutative
          self.appendOp(OpBinaryImm(base_op,dst,src1,src2, val_type = val_type))
        elif base_op in ["shl","lshr","ashr"]:
          if type(src1) != StackLocation:
            raise Exception(f"left operand of {base_op}i has to be StackLocation")
          self.appendOp(OpShiftImm(base_op,dst,src1,src2, val_type = val_type))
        else:
          raise Exception("unsupported operation for immediate: "+base_op)
      self.appendOp(OpBinary(base_op,dst,src1,src2, cmp_type = cmp_type,val_type = val_type))
    elif op_code.startswith("neg.") or op_code.startswith("not."):
      base_op, val_type = op_code.split('.')
      val_type = parseValType(val_type)
      dst = parseLoc(args[0])
      src = parseLoc(args[1])
      self.appendOp(OpUnary(base_op, dst, src,val_type = val_type))
    elif op_code.startswith("cvts.") or op_code.startswith("cvtu."):
      base_op, src_type, dst_type = op_code.split('.')
      src_type = parseValType(src_type)
      dst_type = parseValType(dst_type)
      signed = base_op == "cvts"
      dst = parseLoc(args[0])
      src = parseLoc(args[1])
      self.appendOp(OpCvt(dst, src,src_type = src_type,signed = signed,dst_type = dst_type))
    ## TODO? seperate op-code for long-jump/long-call `ljmp`(?)
    elif op_code == "jmp" or op_code == "call" or op_code == "jmp.abs" or op_code == "call.abs":
      target = parseArg(args[0]) # TODO check target range
      self.appendOp(OpJmp(op_code, target))
    elif op_code == "ret":
      self.appendOp(OpRet())
    elif op_code == "jz" or op_code == "jnz":
      target = parseArg(args[1]) # TODO check target range
      if args[0] == "pop":
        self.appendOp(OpJmp(op_code + " pop", target))
      else:
        arg = parseLoc(args[0])
        self.appendOp(OpJmpIf(op_code, arg, target))
    elif op_code == "copy":
      loc1 = parseLoc(args[0])
      loc2 = parseLoc(args[1])
      self.appendOp(OpCopy(loc1, loc2, drop_count = 0))
    elif op_code.startswith("copy.drop"):
      drop_count = int(op_code[len("copy.drop"):])
      loc1 = parseLoc(args[0])
      loc2 = parseLoc(args[1])
      self.appendOp(OpCopy(loc1, loc2, drop_count = drop_count))
    elif op_code == "swap":
      loc1 = parseLoc(args[0])
      loc2 = parseLoc(args[1])
      self.appendOp(OpSwap(loc1, loc2))
    ## TODO: remaining stack modifications
    elif op_code == "alloc" or op_code == "dealloc":
      count = parseInt(args[0])
      if op_code[0] == "d":
        count = -count
      self.appendOp(OpAlloc(count))
    elif op_code.startswith("data."):
      val_type = op_code[len("data."):]
      args = [elt for arg in args for elt in parseData(arg,val_type)]
      has_label_arg = any(type(arg) == Label for arg in args)
      self.has_label |= has_label_arg
      self.appendOp(OpData(val_type, args, has_label = has_label_arg))
    elif op_code == "!align":
      alignment = parseInt(args[0])
      if (alignment & -alignment) != alignment:
        raise Exception("alignment has to be a power of 2: "+alignment)
      self.appendOp(OpAlign(alignment))
    elif op_code == "!section":
      section_name = args[0]
      if section_name not in ["code","ro_data","rw_data"]:
        raise Exception("unsupported section name: "+section_name)
      self.section = section_name
    elif op_code == "!start":
      self.appendOp(OpStart())
    elif op_code[0] == ':':
      self.appendOp(OpLabel(op_code[1:]))
    else:
      raise Exception("unknown op_code: "+op_code)

def parse(code):
  src = SourceFile()
  for line in code.split('\n'):
    src.parseLine(line)
  return src

def parseFile(srcFile="src.txt"):
  ## TODO: parse into program-class, split ops into sections in parse phase
  with open(srcFile,mode="r") as f:
    src = parse(f.read())
  print(*src.code,sep='\n')
  print()
  print(*src.ro_data,sep='\n')
  print()
  print(*src.rw_data,sep='\n')
  if src.has_label:
    ## 0. set all label offsets to 0
    ## 1. go through program and compute addresses of operations (relative to section base)
    ## 2. assign values to labels
    ## 3. if operation changed size go back to 1
    ## 4. replace labels with their integer values
    raise Exception("label-resolving is not yet implemented")
  prog = Program()
  for op in src.code:
    op.generate(prog)
  for op in src.ro_data:
    op.generate(prog)
  for op in src.rw_data:
    op.generate(prog)
  return prog

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
    prog = parseFile()
    print([hex(op)for op in prog.code])
    ## file-format
    ## [version][ip][code-addr][code-size][ro-addr][ro-data-size][rw-addr][rw-data-size][sp][stack-size]
    stack_pointer = 0x1_0000_0000
    stack_size = 0x10_0000
    code_offset = 0
    with open(out,mode="wb") as f:
        writeU64(f,0) ## reserved
        writeU64(f,prog.start+code_offset)
        writeU64(f,code_offset) ## code-addr
        writeU64(f,(len(prog.code)+1)//2)
        writeU64(f,0x1_0000_0000_0000) ## ro-data-addr
        writeU64(f,(len(prog.ro_data)+7)//8) ## ro-data-size
        writeU64(f,0x2_0000_0000_0000) ## rw-data-addr
        writeU64(f,(len(prog.rw_data)+7)//8) ## rw-data-addr
        writeU64(f,stack_pointer) ## sp
        writeU64(f,stack_size) ## stack-size
        writeU32s(f,prog.code) ## code
        if len(prog.code) & 1: ## padding
          writeU32(f,0)
        f.write(bytes(prog.ro_data))
        if len(prog.ro_data) % 8 != 0: ## padding
          f.write(bytes(0 for _ in range(8-(len(prog.ro_data)%8))))
        f.write(bytes(prog.rw_data))

generate()
