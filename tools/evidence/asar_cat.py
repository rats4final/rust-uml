import json, struct, sys
p = sys.argv[1]
with open(p, 'rb') as f:
    f.read(4); hsize = struct.unpack('<I', f.read(4))[0]
    f.read(4); jlen = struct.unpack('<I', f.read(4))[0]
    hdr = json.loads(f.read(jlen))
    base = 8 + hsize
    for path in sys.argv[2:]:
        n = hdr
        for part in path.strip('/').split('/'): n = n['files'][part]
        f.seek(base + int(n['offset'])); data = f.read(n['size'])
        sys.stdout.write(f"\n===== {path} =====\n"); sys.stdout.write(data.decode('utf-8','replace'))
