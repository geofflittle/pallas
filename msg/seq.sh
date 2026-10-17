#!/bin/sh
# marks the first, third and sixth commits of the todo list for edit
awk 'BEGIN{n=0} /^pick /{n++; if (n==1||n==3||n==6) sub(/^pick/, "edit")} {print}' "$1" > "$1.new" && mv "$1.new" "$1"
