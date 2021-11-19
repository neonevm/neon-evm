#!/bin/bash

# $1 - filename: account.json, sender.json or collateral.json
# $2 - number of files
# $3 - destination dir (will be created)

mkdir $3

split -l$((`wc -l < $1`/$2)) $1 $3/$1 -da2

for i in $3/*.json0*
do
	echo $i
	mv $i `echo $i | sed 's/json0/json/'`
done

