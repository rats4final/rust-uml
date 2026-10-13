#!/usr/bin/env bash
# Rebuild examples/structures.mdj — one diagram per UML structural diagram type
# (package, component, deployment, object, composite structure, profile) —
# using only rust-uml-cli commands.
#
#   examples/build-structures.sh [path/to/rust-uml-cli]
#
# Without an argument it uses `cargo run -p cli`. Needs python3 (to read JSON results).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
out="$here/structures.mdj"
if [ $# -ge 1 ]; then cli=("$1"); else cli=(cargo run -q --manifest-path "$here/../Cargo.toml" -p cli --); fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
file="$work/structures.mdj"
"${cli[@]}" new "$file" > /dev/null

# The diagram commands act on; set by `diagram`.
current=""
run() {
  if [ -n "$current" ]; then
    "${cli[@]}" run "$file" "$1" "$2" -o "$file" -d "$current" 2> /dev/null
  else
    "${cli[@]}" run "$file" "$1" "$2" -o "$file" 2> /dev/null
  fi
}
get() { python3 -I -c 'import json,sys; print(json.load(sys.stdin)[sys.argv[1]])' "$1"; }
set_field() { run element.set "{\"id\":\"$1\",\"key\":\"$2\",\"value\":$3}" > /dev/null; }

diagram() { # type name: add a diagram and make it current
  current=""
  local id; id="$(run diagram.add "{\"type\":\"$1\"}" | get id)"
  set_field "$id" name "\"$2\""
  current="$id"
}
node() { # kind x y name [container-view] -> "model view"
  local params="{\"kind\":\"$1\",\"x\":$2,\"y\":$3"
  if [ $# -ge 5 ]; then params+=",\"container\":\"$5\""; fi
  local r; r="$(run node.create "$params}")"
  local m v; m="$(echo "$r" | get model)"; v="$(echo "$r" | get view)"
  if [ "$m" != "None" ]; then set_field "$m" name "\"$4\""; else set_field "$v" text "\"$4\""; fi
  echo "$m $v"
}
edge() { # kind tail-view head-view -> model
  run edge.create "{\"kind\":\"$1\",\"tail\":\"$2\",\"head\":\"$3\"}" | get model
}
resize() { run view.resize "{\"view\":\"$1\",\"width\":$2,\"height\":$3}" > /dev/null; }
feature() { # owner kind name -> id
  local id; id="$(run feature.add "{\"classifier\":\"$1\",\"kind\":\"$2\"}" | get id)"
  set_field "$id" name "\"$3\""
  echo "$id"
}

# --- Package diagram -------------------------------------------------------
diagram UMLPackageDiagram Packages
read -r _ domain_v < <(node UMLPackage 40 40 domain)
resize "$domain_v" 220 140
node UMLClass 70 90 Order "$domain_v" > /dev/null
read -r _ billing_v < <(node UMLSubsystem 340 40 billing)
read -r _ app_v < <(node UMLModel 340 220 shop)
read -r _ invoice_v < <(node UMLClass 60 260 Invoice)
edge UMLDependency "$domain_v" "$billing_v" > /dev/null
edge UMLContainment "$invoice_v" "$billing_v" > /dev/null
edge UMLDependency "$app_v" "$billing_v" > /dev/null

# --- Component diagram -----------------------------------------------------
diagram UMLComponentDiagram Components
read -r _ shop_v < <(node UMLComponent 40 80 WebShop)
read -r _ http_v < <(node UMLPort 180 100 http "$shop_v")
read -r _ pay_v < <(node UMLComponent 360 80 Payments)
read -r _ api_v < <(node UMLPort 360 100 api "$pay_v")
edge UMLConnector "$http_v" "$api_v" > /dev/null
read -r _ ipay_v < <(node UMLInterface 380 240 IPayment)
edge UMLInterfaceRealization "$pay_v" "$ipay_v" > /dev/null
edge UMLDependency "$shop_v" "$ipay_v" > /dev/null
read -r _ jar_v < <(node UMLArtifact 40 260 shop.jar)
edge UMLComponentRealization "$jar_v" "$shop_v" > /dev/null
read -r _ note_v < <(node Note 600 80 "Ports sit on their component's border; connectors join them.")
resize "$note_v" 150 70
edge NoteLink "$note_v" "$pay_v" > /dev/null

# --- Deployment diagram ----------------------------------------------------
diagram UMLDeploymentDiagram Deployment
read -r _ app_node_v < <(node UMLNode 40 60 AppServer)
resize "$app_node_v" 220 150
node UMLArtifact 70 120 webshop.war "$app_node_v" > /dev/null
read -r _ db_v < <(node UMLNode 400 60 DBServer)
edge UMLCommunicationPath "$app_node_v" "$db_v" > /dev/null
read -r _ sql_v < <(node UMLArtifact 410 280 schema.sql)
edge UMLDeployment "$sql_v" "$db_v" > /dev/null

# --- Object diagram --------------------------------------------------------
diagram UMLObjectDiagram Objects
read -r order order_v < <(node UMLObject 40 60 o42)
set_field "$order" classifier '"Order"'
set_field "$(feature "$order" slot id)" value '"42"'
set_field "$(feature "$order" slot status)" value '"paid"'
read -r cust cust_v < <(node UMLObject 360 60 alice)
set_field "$cust" classifier '"Customer"'
edge UMLLink "$order_v" "$cust_v" > /dev/null
read -r inst inst_v < <(node UMLNodeInstance 40 240 prod1)
set_field "$inst" classifier '"AppServer"'

# --- Composite structure diagram ------------------------------------------
diagram UMLCompositeStructureDiagram Composite
read -r car car_v < <(node UMLClass 40 40 Car)
resize "$car_v" 360 170
# Parts are drawn inside; hide the lists that would repeat them as text.
set_field "$car_v" suppressAttributes true
set_field "$car_v" suppressOperations true
read -r engine engine_v < <(node UMLPart 70 110 engine "$car_v")
set_field "$engine" type '"Engine"'
read -r wheels wheels_v < <(node UMLPart 240 110 wheels "$car_v")
set_field "$wheels" type '"Wheel"'
set_field "$wheels" multiplicity '"4"'
edge UMLConnector "$engine_v" "$wheels_v" > /dev/null
node UMLPort 400 180 controls "$car_v" > /dev/null
node UMLCollaboration 480 60 Observer > /dev/null

# --- Profile diagram -------------------------------------------------------
diagram UMLProfileDiagram Profile
read -r _ meta_v < <(node UMLMetaClass 300 60 Class)
read -r _ entity_v < <(node UMLStereotype 40 60 Entity)
edge UMLExtension "$entity_v" "$meta_v" > /dev/null

# --- Information flow diagram ----------------------------------------------
diagram UMLInformationFlowDiagram "Payment information"
read -r _ cust_v < <(node UMLActor 40 60 Customer)
read -r _ shop_v < <(node UMLClass 260 60 Shop)
read -r _ bank_v < <(node UMLClass 520 60 Bank)
node UMLInformationItem 260 220 PaymentDetails > /dev/null
set_field "$(edge UMLInformationFlow "$cust_v" "$shop_v")" name '"card details"'
set_field "$(edge UMLInformationFlow "$shop_v" "$bank_v")" name '"charge request"'

mv "$file" "$out"
echo "wrote $out"
