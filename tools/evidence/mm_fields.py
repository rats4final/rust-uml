import json, struct, sys
p = sys.argv[1]; want = sys.argv[2].split(',')
with open(p, 'rb') as f:
    f.read(4); hsize = struct.unpack('<I', f.read(4))[0]
    f.read(4); jlen = struct.unpack('<I', f.read(4))[0]
    hdr = json.loads(f.read(jlen)); base = 8 + hsize
    def get(path):
        n = hdr
        for part in path.strip('/').split('/'): n = n['files'][part]
        f.seek(base + int(n['offset'])); return json.loads(f.read(n['size']))
    mm = {}
    for pth in ['/resources/default/metamodel.json','/extensions/essential/uml/metamodel.json']: mm.update(get(pth))
    for w in want:
        v = mm.get(w)
        if not v: print(w, 'MISSING'); continue
        attrs = ', '.join(f"{a['name']}:{a['kind']}:{a['type']}" + (f"={a['default']}" if 'default' in a else '') for a in v.get('attributes',[]))
        print(f"{w} <{v.get('super')}> {v.get('kind')} :: {attrs}" + (f" literals={v.get('literals')}" if v.get('literals') else ''))
