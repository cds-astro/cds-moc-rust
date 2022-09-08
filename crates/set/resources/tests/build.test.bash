#!/bin/bash

list="moclist.txt"

cp_mocs(){
  [[ ! -d data ]] && { mkdir data; }

  for f in $(awk 'NR % 100 == 5' ../../local_resource/moclist.txt | cut -d , -f 2); do
    cp $f data
  done
}

build_list_file(){
  local i=0
  for f in $(ls data/*.fits); do
    i=$((i+1))
    echo "$i,${f}"

  done
}


# cp_mocs
build_list_file > ${list}

