import json, struct, sys, re
p, pat = sys.argv[1], re.compile(sys.argv[2])
with open(p, 'rb') as f:
    f.read(4); hsize = struct.unpack('<I', f.read(4))[0]
    f.read(4); jlen = struct.unpack('<I', f.read(4))[0]
    hdr = json.loads(f.read(jlen)); base = 8 + hsize
    def walk(n, path):
        for name, c in n.get('files', {}).items():
            full = path + '/' + name
            if 'files' in c:
                if name != 'node_modules': yield from walk(c, full)
            elif name.endswith(('.js','.json','.html')) and 'offset' in c:
                yield full, c
    for full, c in walk(hdr, ''):
        f.seek(base + int(c['offset'])); txt = f.read(c['size']).decode('utf-8','replace')
        for i, line in enumerate(txt.splitlines(), 1):
            if pat.search(line): print(f"{full}:{i}: {line.strip()[:200]}")
