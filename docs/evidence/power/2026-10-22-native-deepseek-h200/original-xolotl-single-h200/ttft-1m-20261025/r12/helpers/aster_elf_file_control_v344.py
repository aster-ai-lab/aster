"""Compare CUDA control objects exactly, allowing only one typed ELF STT_FILE filename."""
import hashlib
import struct


def sections(raw):
    if len(raw)>8*1024**2 or raw[:6]!=b'\x7fELF\x02\x01' or len(raw)<64:
        raise ValueError('bounded ELF64 little-endian object required')
    offset=struct.unpack_from('<Q',raw,40)[0]
    size,count,names=struct.unpack_from('<HHH',raw,58)
    if size!=64 or not 0<count<=1024 or names>=count or offset+size*count>len(raw):
        raise ValueError('bounded ELF section headers required')
    result=[]
    for i in range(count):
        row=struct.unpack_from('<IIQQQQIIQQ',raw,offset+i*size)
        if row[1]!=8 and row[4]+row[5]>len(raw):raise ValueError('ELF section outside object')
        result.append(row)
    return result


def normalize_file_symbol(reference,control):
    a=sections(reference);b=sections(control)
    if len(reference)!=len(control) or a!=b or reference[:64]!=control[:64]:
        raise ValueError('control layout/header differs')
    differences=[]
    for index,row in enumerate(a):
        if row[1]!=2:continue # SHT_SYMTAB, identified by its declared ELF type
        if row[9]!=24 or row[5]%24 or row[6]>=len(a):raise ValueError('ELF symbol table shape differs')
        strings=a[row[6]]
        if strings[1]!=3:raise ValueError('ELF symbol string table type differs')
        if reference[row[4]:row[4]+row[5]]!=control[row[4]:row[4]+row[5]]:
            raise ValueError('symbol definitions differ')
        name_users=[]
        for offset in range(row[4],row[4]+row[5],24):
            name,info,other,shndx,value,size=struct.unpack_from('<IBBHQQ',reference,offset)
            name_users.append((name,info))
            if (info&15)!=4:continue # STT_FILE
            if info!=4 or other!=0 or shndx!=0xfff1 or value!=0 or size!=0:
                raise ValueError('local absolute filename-only symbol required')
            if name>=strings[5]:raise ValueError('file symbol name outside string table')
            start=strings[4]+name;end=strings[4]+strings[5]
            first=reference[start:end].split(b'\0',1)[0]
            second=control[start:end].split(b'\0',1)[0]
            if first==second:continue
            if not 0<len(first)<=256 or len(first)!=len(second):raise ValueError('equal bounded file-symbol lengths required')
            differences.append(dict(start=start,offset_in_strings=name,length=len(first),original=first,control=second,
                                    symbol_table=index,users=name_users))
        # Retain all name users, including symbols after the file entry.
        for diff in differences:
            if diff['symbol_table']==index:diff['users']=name_users
    if len(differences)!=1:raise ValueError('exactly one differing STT_FILE bookkeeping name required')
    diff=differences[0];lo=diff['offset_in_strings'];hi=lo+diff['length']
    if any(lo<=offset<=hi and info!=4 for offset,info in diff['users']):
        raise ValueError('file string overlaps a non-file symbol name')
    normalized=bytearray(control)
    normalized[diff['start']:diff['start']+diff['length']]=diff['original']
    normalized=bytes(normalized)
    if normalized!=reference:raise ValueError('other code, data, relocation or metadata bytes differ')
    return normalized,dict(original_raw_sha256=hashlib.sha256(reference).hexdigest(),control_raw_sha256=hashlib.sha256(control).hexdigest(),
        normalized_sha256=hashlib.sha256(normalized).hexdigest(),all_other_object_bytes_equal=True,
        differing_STT_FILE_symbols=1,original_file_symbol=diff['original'].decode(),control_file_symbol=diff['control'].decode())
