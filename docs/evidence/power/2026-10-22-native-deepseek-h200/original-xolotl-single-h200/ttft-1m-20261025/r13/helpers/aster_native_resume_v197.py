"""Pinned GEN protocol: zero reuse has no RESUME line; DONE independently states reuse."""
def validate_resume(metadata,done,index,prompt_tokens=1047544,output_tokens=128):
    if type(index) is not int or index not in [0,1,2,3]:raise ValueError('exact four-GEN ordinal differs')
    fields=done.split()
    if len(fields)<9 or fields[0]!='DONE' or int(fields[1])!=output_tokens or int(fields[2])!=prompt_tokens or fields[5]!='length':
        raise ValueError('exact full-input/output DONE differs')
    expected=0 if index==0 else prompt_tokens
    lines=[line for line in metadata if line.startswith('RESUME')]
    required=[] if expected==0 else ['RESUME '+str(expected)]
    if lines!=required or int(fields[8])!=expected:raise ValueError('exact conditional RESUME and DONE reuse differ')
    return expected
