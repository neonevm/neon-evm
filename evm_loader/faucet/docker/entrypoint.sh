#!/bin/sh
# This script is intended to run in a container automatically.
# It's pointless to try launch it out of that context.

exec faucet --config $HOME/faucet.conf run
