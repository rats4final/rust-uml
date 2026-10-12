import json, struct, sys
p = sys.argv[1]
with open(p, 'rb') as f:
    f.read(4); hsize = struct.unpack('<I', f.read(4))[0]
    f.read(4); jlen = struct.unpack('<I', f.read(4))[0]
    hdr = json.loads(f.read(jlen))
def walk(node, path, depth):
    for name, n in node.get('files', {}).items():
        full = path + '/' + name
        if 'files' in n:
            cnt = sum(1 for _ in n['files'])
            if depth < int(sys.argv[2]): print(f"{full}/  ({cnt} entries)"); walk(n, full, depth+1)
        elif depth < int(sys.argv[2]):
            print(f"{full}  {n.get('size',0)}")
walk(hdr, '', 0)
