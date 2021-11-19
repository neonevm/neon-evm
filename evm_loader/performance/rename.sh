#!/bin/bash

for i in $1/*.json0*
do
	echo $i
	mv $i `echo $i | sed 's/json0/json/'`
done