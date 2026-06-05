# SPDX-FileCopyrightText: 2026 Chen Linxuan <me@black-desk.cn>

# SPDX-License-Identifier: MIT

all:

install:
	if [ -n "$$VCPKG_ROOT" ]; then \
		cargo install --path rsime --features cli,bundled-vcpkg; \
	else \
		cargo install --path rsime --features cli; \
	fi
