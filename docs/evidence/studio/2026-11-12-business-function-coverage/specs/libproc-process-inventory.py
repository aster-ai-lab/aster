#!/usr/bin/env python3
"""Minimal ps replacement using libproc, for sessions where /bin/ps exec is denied.

Only supports the process-inventory shapes agent-desktop needs: pid, ppid,
comm/command, with or without headers. Not a general-purpose ps.
"""
import ctypes
import sys

lib = ctypes.CDLL("/usr/lib/libproc.dylib")
lib.proc_listallpids.argtypes = [ctypes.c_void_p, ctypes.c_int]
lib.proc_listallpids.restype = ctypes.c_int
lib.proc_pidinfo.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_uint64, ctypes.c_void_p, ctypes.c_int]
lib.proc_pidinfo.restype = ctypes.c_int


class ProcBSDInfo(ctypes.Structure):
    _fields_ = [("pbi_flags", ctypes.c_uint32), ("pbi_status", ctypes.c_uint32),
                ("pbi_xstatus", ctypes.c_uint32), ("pbi_pid", ctypes.c_uint32),
                ("pbi_ppid", ctypes.c_uint32), ("pbi_uid", ctypes.c_uint32),
                ("pbi_gid", ctypes.c_uint32), ("pbi_ruid", ctypes.c_uint32),
                ("pbi_rgid", ctypes.c_uint32), ("pbi_svuid", ctypes.c_uint32),
                ("pbi_svgid", ctypes.c_uint32), ("rfu_1", ctypes.c_uint32),
                ("pbi_comm", ctypes.c_char * 16), ("pbi_name", ctypes.c_char * 32),
                ("pbi_nfiles", ctypes.c_uint32), ("pbi_pgid", ctypes.c_uint32),
                ("pbi_pjobc", ctypes.c_uint32), ("e_tdev", ctypes.c_uint32),
                ("e_tpgid", ctypes.c_uint32), ("pbi_nice", ctypes.c_int32),
                ("pbi_start_tvsec", ctypes.c_uint64), ("pbi_start_tvusec", ctypes.c_uint64)]


def rows():
    buf = (ctypes.c_int * 8192)()
    count = lib.proc_listallpids(buf, ctypes.sizeof(buf))
    out = []
    for i in range(max(count, 0)):
        pid = buf[i]
        info = ProcBSDInfo()
        if lib.proc_pidinfo(pid, 3, 0, ctypes.byref(info), ctypes.sizeof(info)) > 0:
            name = info.pbi_comm.decode("utf-8", "replace") or "?"
            out.append((pid, info.pbi_ppid, name))
    return out


def main():
    args = " ".join(sys.argv[1:])
    fields = []
    for key in ("pid", "ppid", "comm", "command", "args"):
        if key in args:
            fields.append(key)
    if not fields:
        fields = ["pid", "comm"]
    no_header = "=" in args
    if not no_header:
        print(" ".join(f.upper() for f in fields))
    for pid, ppid, name in rows():
        cells = []
        for f in fields:
            if f == "pid":
                cells.append(str(pid))
            elif f == "ppid":
                cells.append(str(ppid))
            else:
                cells.append(name)
        print(" ".join(cells))


if __name__ == "__main__":
    main()
