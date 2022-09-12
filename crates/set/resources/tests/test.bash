#!/bin/bash

list="moclist.txt"
mocset="mocset.bin"
expected_dir="expected"

# 1: diff result; 2: context
assert_empty(){
  if [ "$1" != "" ]; then
    echo "Error in cmd '$2'. Diff with expected: \n$1";
    exit 1;
  fi
}


# 1: actual; 2: expeced; 3: context
assert_eq(){
  if [ "$1" != "$2" ]; then
    echo "Error in '$3'. Actual: '$1'. Expected: '$2'";
    exit 1; 
  fi
}

build_mocset(){
  [[ -f ${mocset} ]] && { rm ${mocset}; }
  RUST_BACKTRACE=1 cargo run --release -- make --n128 3 --moc-list ${list} --delimiter , ${mocset}
}

query_cone_valid(){
  local cmd="cargo run --release -- query ${mocset} cone 0.0 +0.0 0.1"
  local actual=$(${cmd} | tr '\n' ' ' | sed -r 's/ +$//')
  local expected="id 161"
  assert_eq "${actual}" "${expected}" "query cone 0.0 +0.0 0.1"
}

query_cone_negdec_valid(){
  local cmd="cargo run --release -- query ${mocset} cone 10.0 -12.0 0.1"
  local actual=$(${cmd} | tr '\n' ' ' | sed -r 's/ +$//')
  local expected="id"
  assert_eq "${actual}" "${expected}" "query cone 10.0 -12.0 0.1"
}


query_cone_valid_prec6(){
  local cmd="cargo run --release -- query ${mocset} cone 0.0 +0.0 0.1 --precision 6"
  local actual=$(${cmd} | tr '\n' ' ' | sed -r 's/ +$//')
  local expected="id 161"
  assert_eq "${actual}" "${expected}" "query cone 0.0 +0.0 0.1 --precision 6"
}


query_cone_valid_with_coverage(){
  local cmd="cargo run --release -- query --print-coverage ${mocset} cone 0.0 +0.0 0.1"
  local actual=$(${cmd} | tr '\n' ' ' | sed -r 's/ +$//')
  local expected="id,moc_coverage 161,1.396761e-1"
  assert_eq "${actual}" "${expected}" "query cone 0.0 +0.0 0.1"
}

query_cone_deprecated_nodeprect(){
  local cmd="cargo run --release -- query ${mocset} cone 0.0 +0.0 0.1"
  local actual=$(${cmd} | tr '\n' ' ' | sed -r 's/ +$//')
  local expected="id"
  assert_eq "${actual}" "${expected}" "query cone 0.0 +0.0 0.1"
}

query_cone_deprecated_withdeprect(){
  local cmd="cargo run --release -- query --add-deprecated ${mocset} cone 0.0 +0.0 0.1"
  local actual=$(${cmd} | tr '\n' ' ' | sed -r 's/ +$//')
  local expected="id 161"
  assert_eq "${actual}" "${expected}" "query cone 0.0 +0.0 0.1"
}

query_cone_removed_nodeprec(){
  local cmd="cargo run --release -- query ${mocset} cone 0.0 +0.0 0.1"
  local actual=$(${cmd} | tr '\n' ' ' | sed -r 's/ +$//')
  local expected="id"
  assert_eq "${actual}" "${expected}" "query cone 0.0 +0.0 0.1"
}

query_cone_removed_withdeprec(){
  local cmd="cargo run --release -- query --add-deprecated ${mocset} cone 0.0 +0.0 0.1"
  local actual=$(${cmd} | tr '\n' ' ' | sed -r 's/ +$//')
  local expected="id"
  assert_eq "${actual}" "${expected}" "query cone 0.0 +0.0 0.1"
}


list1(){
  local cmd="cargo run --release -- list mocset.bin"
  local expected="${expected_dir}/mocset.query.list1.txt"
  local diff=$(${cmd} | diff - ${expected})
  assert_empty "${diff}" "${cmd}"
}

list2(){
  local cmd="cargo run --release -- list mocset.bin"
  local expected="${expected_dir}/mocset.query.list2.txt"
  local diff=$(${cmd} | diff - ${expected})
  assert_empty "${diff}" "${cmd}"
}

list3(){
  local cmd="cargo run --release -- list mocset.bin"
  local expected="${expected_dir}/mocset.query.list3.txt"
  local diff=$(${cmd} | diff - ${expected})
  assert_empty "${diff}" "${cmd}"
}



build_mocset
list1
query_cone_valid
query_cone_valid_with_coverage
query_cone_valid_prec6
query_cone_negdec_valid
#cargo run --release list --ranges mocset.bin
cargo run --release purge mocset.bin
#cargo run --release list --ranges mocset.bin
list1
query_cone_valid
query_cone_valid_with_coverage
list1
cargo run --release chgstatus mocset.bin 50 deprecated
list2
cargo run --release chgstatus mocset.bin 50 valid
list1
cargo run --release chgstatus mocset.bin 50 removed 
list3
cargo run --release chgstatus mocset.bin 161 deprecated
query_cone_deprecated_nodeprect
query_cone_deprecated_withdeprect
cargo run --release chgstatus mocset.bin 161 removed
query_cone_removed_nodeprec
query_cone_removed_withdeprec
cargo run --release append mocset.bin -161 data/CDS_J_ApJ_811_30_table3.fits
cargo run --release chgstatus mocset.bin 161 valid
cargo run --release purge mocset.bin
query_cone_valid
query_cone_valid_with_coverage
rm ${mocset}

echo "Everything seems OK :)"

