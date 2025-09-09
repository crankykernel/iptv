# SPDX-License-Identifier: MIT
# SPDX-FileCopyrightText: (C) 2025 Cranky Kernel <crankykernel@proton.me>

default:

fix:
	cargo clippy --fix --allow-dirty
	cargo fmt

check:
	cargo check
	cargo clippy
