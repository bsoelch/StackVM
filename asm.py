#!/usr/bin/python

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
0x00000b20, # jump to start
0x00000000, # loadi dst:0 val:0
0x00001000, # loadi dst:0 val:1
0x000c2033, # cmpi.i64 lt dst:0 swap src:3 val:0
0x00004027, # jz.drop src:1 dst:4
0x00101153, # add.i64 dst:1 src1:1 src2:2
0xffff233b, # addi.i64 dst:3 src:3 val:-1
0x0001008c, # swap arg1:1 arg2:2
0xfffffa22, # jmp dst:-6
0x00401289, # copy drop: 2 dst:1 src:2
0xffffff23, # ret
0x00006000, # loadi dst:0 val:5
0x00000121, # call_abs val:1
    ]
    print([hex(op)for op in ops])
    ## file-format
    ## [version][code-size][ro-data-size][rw-data-size]
    with open(out,mode="wb") as f:
        writeU64(f,0) ## reserved
        writeU64(f,(len(ops)+1)//2)
        writeU64(f,0) ## no data
        writeU64(f,0) ## no data
        writeU32s(f,ops) ## code
        if len(ops) & 1: ## padding
          writeU32(f,0)

generate()
