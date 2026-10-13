#!/usr/bin/env bash
# Rebuild examples/shapes.mdj using only rust-uml-cli commands.
#
#   examples/build-shapes.sh [path/to/rust-uml-cli]
#
# Without an argument it uses `cargo run -p cli`. Needs python3 (to read JSON results).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
out="$here/shapes.mdj"
if [ $# -ge 1 ]; then cli=("$1"); else cli=(cargo run -q --manifest-path "$here/../Cargo.toml" -p cli --); fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
file="$work/shapes.mdj"
"${cli[@]}" new "$file" > /dev/null

# Run one command, save the model, print the JSON result.
run() { "${cli[@]}" run "$file" "$1" "$2" -o "$file" 2> /dev/null; }
# Read one field from a JSON result on stdin.
get() { python3 -I -c 'import json,sys; print(json.load(sys.stdin)[sys.argv[1]])' "$1"; }
set_field() { run element.set "{\"id\":\"$1\",\"key\":\"$2\",\"value\":$3}" > /dev/null; }

node() { # kind x y name -> "model view"
  local r; r="$(run node.create "{\"kind\":\"$1\",\"x\":$2,\"y\":$3}")"
  local m v; m="$(echo "$r" | get model)"; v="$(echo "$r" | get view)"
  set_field "$m" name "\"$4\""
  echo "$m $v"
}
feature() { # classifier kind name -> id
  local id; id="$(run feature.add "{\"classifier\":\"$1\",\"kind\":\"$2\"}" | get id)"
  set_field "$id" name "\"$3\""
  echo "$id"
}

read -r shape shape_v < <(node Interface 260 30 Shape)
read -r circle circle_v < <(node Class 120 220 Circle)
read -r square square_v < <(node Class 400 220 Square)
read -r canvas canvas_v < <(node Class 620 40 Canvas)
read -r color _ < <(node Enumeration 640 240 Color)

area="$(feature "$shape" operation area)"
ret="$(run parameter.add "{\"operation\":\"$area\",\"direction\":\"return\"}" | get id)"
set_field "$ret" type '"double"'

radius="$(feature "$circle" attribute radius)"
set_field "$radius" type '"double"'; set_field "$radius" visibility '"private"'
side="$(feature "$square" attribute side)"
set_field "$side" type '"double"'; set_field "$side" visibility '"private"'
count="$(feature "$canvas" attribute count)"
set_field "$count" type '"int"'; set_field "$count" isStatic true
for literal in RED GREEN BLUE; do feature "$color" literal "$literal" > /dev/null; done

run edge.create "{\"kind\":\"Interface Realization\",\"tail\":\"$circle_v\",\"head\":\"$shape_v\"}" > /dev/null
run edge.create "{\"kind\":\"Interface Realization\",\"tail\":\"$square_v\",\"head\":\"$shape_v\"}" > /dev/null
assoc="$(run edge.create "{\"kind\":\"Composition\",\"tail\":\"$canvas_v\",\"head\":\"$shape_v\"}" | get model)"
end2="$(run model.get "{\"id\":\"$assoc\"}" | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["fields"]["end2"]["child"])')"
set_field "$end2" name '"shapes"'
set_field "$end2" multiplicity '"0..*"'

mv "$file" "$out"
echo "wrote $out"
