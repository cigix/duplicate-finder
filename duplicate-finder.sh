#!/bin/bash

THIS_SCRIPT=$(realpath "$BASH_SOURCE")
THIS_DIR=$(dirname "$THIS_SCRIPT")

if [ ! -d "$THIS_DIR"/venv ]
then
    python3 -m venv "$THIS_DIR"/venv
    source "$THIS_DIR"/venv/bin/activate
    pip install -r "$THIS_DIR"/requirements.txt
else
    source "$THIS_DIR"/venv/bin/activate
fi

"$THIS_DIR"/duplicate-finder.py "$@"
