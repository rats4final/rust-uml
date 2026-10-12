import json, struct, sys, collections
p = sys.argv[1]
with open(p, 'rb') as f:
    f.read(4); hsize = struct.unpack('<I', f.read(4))[0]
    f.read(4); jlen = struct.unpack('<I', f.read(4))[0]
    hdr = json.loads(f.read(jlen)); base = 8 + hsize
    def get(path):
        n = hdr
        for part in path.strip('/').split('/'): n = n['files'][part]
        f.seek(base + int(n['offset'])); return json.loads(f.read(n['size']))
    paths = ['/resources/default/metamodel.json'] + [f'/extensions/essential/{e}/metamodel.json' for e in
             ['uml','sysml','bpmn','erd','flowchart','dfd','c4','aws','azure','gcp','mindmap','wireframe']]
    kinds_all = collections.Counter(); total = 0
    for pth in paths:
        mm = get(pth); cls = [k for k,v in mm.items() if v.get('kind')=='class']
        views = [k for k in cls if k.endswith('View')]; dg = [k for k in cls if k.endswith('Diagram')]
        for v in mm.values():
            for a in v.get('attributes',[]): kinds_all[a['kind']] += 1
        total += len(cls)
        print(f"{pth.split('/')[-2]:10} classes={len(cls):4} views={len(views):4} diagrams={dg}")
    print('total classes', total, 'attribute kinds', dict(kinds_all))
