#!/bin/bash

cargo modules dependencies \
	--no-sysroot \
	--no-fns \
	--no-traits \
	--no-types \
	--no-uses \
	--lib | dot -Tpng >deps_graph.png

xdg-open deps_graph.png &
