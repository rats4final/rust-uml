#!/usr/bin/env bash
# Rebuild examples/behaviour.mdj — a use case, a statechart and an activity
# diagram — using only rust-uml-cli commands.
#
#   examples/build-behaviour.sh [path/to/rust-uml-cli]
#
# Without an argument it uses `cargo run -p cli`. Needs python3 (to read JSON results).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
out="$here/behaviour.mdj"
if [ $# -ge 1 ]; then cli=("$1"); else cli=(cargo run -q --manifest-path "$here/../Cargo.toml" -p cli --); fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
file="$work/behaviour.mdj"
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
node() { # kind x y [name] [container-view] -> "model view"
  local params="{\"kind\":\"$1\",\"x\":$2,\"y\":$3"
  if [ $# -ge 5 ] && [ -n "$5" ]; then params+=",\"container\":\"$5\""; fi
  local r; r="$(run node.create "$params}")"
  local m v; m="$(echo "$r" | get model)"; v="$(echo "$r" | get view)"
  if [ $# -ge 4 ] && [ -n "$4" ]; then
    # Shapes without a model (time ticks) show their view's text.
    if [ "$m" != "None" ]; then set_field "$m" name "\"$4\""; else set_field "$v" text "\"$4\""; fi
  fi
  echo "$m $v"
}
edge() { # kind tail-view head-view -> model
  run edge.create "{\"kind\":\"$1\",\"tail\":\"$2\",\"head\":\"$3\"}" | get model
}
resize() { run view.resize "{\"view\":\"$1\",\"width\":$2,\"height\":$3}" > /dev/null; }

# --- Use case diagram ------------------------------------------------------
diagram UMLUseCaseDiagram "Use Cases"
read -r _ customer_v < <(node UMLActor 30 120 Customer)
read -r _ clerk_v < <(node UMLActor 560 120 Clerk)
read -r _ shop_v < <(node UMLUseCaseSubject 150 30 Shop)
resize "$shop_v" 340 300
read -r _ order_v < <(node UMLUseCase 220 80 "Place Order" "$shop_v")
read -r _ pay_v < <(node UMLUseCase 220 190 "Pay" "$shop_v")
read -r _ coupon_v < <(node UMLUseCase 330 260 "Apply Coupon" "$shop_v")
edge UMLAssociation "$customer_v" "$order_v" > /dev/null
edge UMLAssociation "$clerk_v" "$order_v" > /dev/null
edge UMLInclude "$order_v" "$pay_v" > /dev/null
edge UMLExtend "$coupon_v" "$pay_v" > /dev/null

# --- Statechart diagram ----------------------------------------------------
diagram UMLStatechartDiagram "Order States"
read -r _ start_v < <(node UMLInitialState 40 60)
read -r _ new_v < <(node UMLState 110 40 New)
read -r _ paying_v < <(node UMLCompositeState 300 20 Paying)
resize "$paying_v" 240 150
read -r _ auth_v < <(node UMLState 330 80 Authorising "$paying_v")
read -r _ hist_v < <(node UMLShallowHistory 480 130 "" "$paying_v")
read -r _ check_v < <(node UMLChoice 200 230)
read -r _ done_v < <(node UMLFinalState 205 330)
read -r _ cancelled_v < <(node UMLState 300 260 Cancelled)
set_field "$(edge UMLTransition "$start_v" "$new_v")" name '"create"'
t="$(edge UMLTransition "$new_v" "$paying_v")"; set_field "$t" name '"checkout"'; set_field "$t" guard '"cart not empty"'
edge UMLTransition "$paying_v" "$check_v" > /dev/null
t="$(edge UMLTransition "$check_v" "$done_v")"; set_field "$t" guard '"paid"'
t="$(edge UMLTransition "$check_v" "$cancelled_v")"; set_field "$t" guard '"declined"'
set_field "$(edge UMLSelfTransition "$new_v" "$new_v")" name '"edit"'

# --- Activity diagram ------------------------------------------------------
diagram UMLActivityDiagram "Fulfilment"
read -r _ sales_v < <(node UMLSwimlaneVert 20 20 Sales)
resize "$sales_v" 220 460
read -r _ store_v < <(node UMLSwimlaneVert 240 20 Warehouse)
resize "$store_v" 260 460
read -r _ init_v < <(node UMLInitialNode 120 60 "" "$sales_v")
read -r _ receive_v < <(node UMLAction 75 110 "Receive Order" "$sales_v")
read -r _ decide_v < <(node UMLDecisionNode 115 190 "" "$sales_v")
read -r _ reject_v < <(node UMLAction 75 260 "Reject Order" "$sales_v")
read -r _ fork_v < <(node UMLForkNode 290 200 "" "$store_v")
read -r _ pick_v < <(node UMLAction 260 250 "Pick Items" "$store_v")
read -r _ invoice_v < <(node UMLAction 380 250 "Send Invoice" "$store_v")
read -r _ join_v < <(node UMLJoinNode 330 330 "" "$store_v")
read -r _ end_v < <(node UMLActivityFinalNode 355 400 "" "$store_v")
read -r _ stop_v < <(node UMLFlowFinalNode 115 360 "" "$sales_v")
edge UMLControlFlow "$init_v" "$receive_v" > /dev/null
edge UMLControlFlow "$receive_v" "$decide_v" > /dev/null
set_field "$(edge UMLControlFlow "$decide_v" "$fork_v")" guard '"in stock"'
set_field "$(edge UMLControlFlow "$decide_v" "$reject_v")" guard '"out of stock"'
edge UMLControlFlow "$reject_v" "$stop_v" > /dev/null
edge UMLControlFlow "$fork_v" "$pick_v" > /dev/null
edge UMLControlFlow "$fork_v" "$invoice_v" > /dev/null
edge UMLControlFlow "$pick_v" "$join_v" > /dev/null
edge UMLControlFlow "$invoice_v" "$join_v" > /dev/null
edge UMLControlFlow "$join_v" "$end_v" > /dev/null

# --- Sequence diagram ------------------------------------------------------
diagram UMLSequenceDiagram "Checkout"
read -r c c_v < <(node UMLLifeline 40 40 customer)
read -r sh sh_v < <(node UMLLifeline 220 40 shop)
read -r p p_v < <(node UMLLifeline 400 40 payments)
for lv in "$c_v" "$sh_v" "$p_v"; do resize "$lv" 100 380; done
msg() { # kind tail head y name -> model
  local m; m="$(run edge.create "{\"kind\":\"$1\",\"tail\":\"$2\",\"head\":\"$3\",\"y\":$4}" | get model)"
  set_field "$m" name "\"$5\""
  echo "$m"
}
msg UMLMessage "$c_v" "$sh_v" 120 checkout > /dev/null
set_field "$(msg UMLMessage "$sh_v" "$p_v" 160 charge)" arguments '"total"'
msg UMLReplyMessage "$p_v" "$sh_v" 210 approved > /dev/null
read -r frag frag_v < <(node UMLCombinedFragment 180 240 "")
resize "$frag_v" 330 150
run feature.add "{\"classifier\":\"$frag\",\"kind\":\"operand\"}" > /dev/null
ops="$(run model.get "{\"id\":\"$frag\"}" | python3 -I -c 'import json,sys; print(" ".join(json.load(sys.stdin)["fields"]["operands"]["children"]))')"
set -- $ops
set_field "$1" guard '"in stock"'
set_field "$2" guard '"else"'
msg UMLAsyncMessage "$sh_v" "$c_v" 305 confirmation > /dev/null
msg UMLAsyncMessage "$sh_v" "$c_v" 372 backorderNotice > /dev/null

# --- Communication diagram -------------------------------------------------
diagram UMLCommunicationDiagram "Checkout (communication)"
read -r _ cc_v < <(node UMLLifeline 60 120 customer)
read -r _ cs_v < <(node UMLLifeline 340 120 shop)
read -r _ cp_v < <(node UMLLifeline 340 300 payments)
comm() { # kind tail head name -> model (a message between lifelines brings its connector)
  run edge.create "{\"kind\":\"$1\",\"tail\":\"$2\",\"head\":\"$3\",\"name\":\"$4\"}" | get model
}
comm UMLForwardMessage "$cc_v" "$cs_v" checkout > /dev/null
connector="$(run diagram.inspect '{}' | python3 -I -c 'import json,sys; print(next(l["view"] for l in json.load(sys.stdin)["lines"] if l["type"] == "UMLConnectorView"))')"
set_field "$(comm UMLForwardMessage "$cs_v" "$cp_v" charge)" arguments '"total"'
comm UMLReverseMessage "$cs_v" "$connector" confirmation > /dev/null

# --- Timing diagram --------------------------------------------------------
diagram UMLTimingDiagram "Door alarm"
read -r _ door_v < <(node UMLLifeline 0 0 door)
read -r _ open_v < <(node UMLTimingState 0 0 Open "$door_v")
read -r _ closed_v < <(node UMLTimingState 0 0 Closed "$door_v")
read -r _ d1_v < <(node UMLTimeSegment 0 0 "" "$closed_v")
node UMLTimeSegment 0 0 "" "$open_v" > /dev/null
node UMLTimeSegment 0 0 "" "$closed_v" > /dev/null
read -r _ alarm_v < <(node UMLLifeline 0 0 alarm)
read -r _ armed_v < <(node UMLTimingState 0 0 Armed "$alarm_v")
read -r _ ringing_v < <(node UMLTimingState 0 0 Ringing "$alarm_v")
node UMLTimeSegment 0 0 "" "$armed_v" > /dev/null
read -r _ a2_v < <(node UMLTimeSegment 0 0 "" "$ringing_v")
node UMLTimeSegment 0 0 "" "$armed_v" > /dev/null
for t in 0 1 2 3; do node UMLTimeTick 0 0 "$t" > /dev/null; done
set_field "$(edge UMLMessage "$d1_v" "$a2_v")" name '"opened"'

# --- Interaction overview diagram ------------------------------------------
diagram UMLInteractionOverviewDiagram "Shopping overview"
ref_of() { # element field -> the id it refers to
  run model.get "{\"id\":\"$1\"}" | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["fields"][sys.argv[1]]["$ref"])' "$2"
}
read -r _ o_init_v < <(node UMLInitialNode 60 90 "")
read -r browse browse_v < <(node UMLInteractionUseInOverview 130 70 browse)
# The use refers to a new interaction; name it for the frame's label.
set_field "$(ref_of "$(ref_of "$browse" target)" refersTo)" name '"Browse Catalogue"'
read -r _ o_decide_v < <(node UMLDecisionNode 185 190 "")
read -r pay pay_v < <(node UMLInteractionInOverview 60 260 pay)
resize "$pay_v" 260 170
pay_dgm="$(ref_of "$pay" target)"
set_field "$pay_dgm" name '"Pay"'
read -r _ o_end_v < <(node UMLActivityFinalNode 420 197 "")
edge UMLControlFlow "$o_init_v" "$browse_v" > /dev/null
edge UMLControlFlow "$browse_v" "$o_decide_v" > /dev/null
set_field "$(edge UMLControlFlow "$o_decide_v" "$pay_v")" guard '"buy"'
set_field "$(edge UMLControlFlow "$o_decide_v" "$o_end_v")" guard '"leave"'
edge UMLControlFlow "$pay_v" "$o_end_v" > /dev/null
# Draw the inline interaction itself (its own sequence diagram).
current="$pay_dgm"
read -r _ pl1_v < <(node UMLLifeline 20 20 customer)
read -r _ pl2_v < <(node UMLLifeline 200 20 payments)
msg UMLMessage "$pl1_v" "$pl2_v" 110 pay > /dev/null
msg UMLReplyMessage "$pl2_v" "$pl1_v" 160 receipt > /dev/null

mv "$file" "$out"
echo "wrote $out"
