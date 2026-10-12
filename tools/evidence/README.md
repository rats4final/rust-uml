# Evidence readers

Small read-only scripts used to produce `docs/evidence.md`. They read an installed StarUML's
`app.asar` (a JSON table of contents followed by file bytes) without extracting or running anything.

```sh
A="/mnt/c/Program Files/StarUML/resources/app.asar"
python3 -I asar_toc.py  "$A" 3                      # list files, 3 levels deep
python3 -I asar_cat.py  "$A" /package.json          # print one member
python3 -I asar_grep.py "$A" 'licenseStatus|PRO'    # regex over StarUML's own JS (skips node_modules)
python3 -I mm_stats.py  "$A"                        # element/view/diagram counts per family
python3 -I mm_fields.py "$A" UMLClass,UMLClassView  # field definitions of metamodel types
```

Output may quote StarUML code. Keep it out of the repository.
