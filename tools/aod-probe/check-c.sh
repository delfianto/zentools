#!/usr/bin/env bash
# Compile and analyze the kernel helper without loading it or using root.
set -euo pipefail

probe_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
kernel_build_dir=$(realpath -- "${KDIR:-/lib/modules/$(uname -r)/build}")

clang-format --dry-run --Werror "$probe_dir"/*.c "$probe_dir"/*.h
perl "$kernel_build_dir/scripts/checkpatch.pl" --no-tree --strict --file \
    "$probe_dir"/*.c "$probe_dir"/*.h

task_build_dir=$(mktemp -d "${TMPDIR:-/tmp}/zentools-c-lint.XXXXXX")
trap 'rm -rf -- "$task_build_dir"' EXIT
cp -- "$probe_dir"/*.c "$probe_dir"/*.h "$probe_dir/Makefile" "$task_build_dir/"

make -C "$kernel_build_dir" M="$task_build_dir" LLVM=1 W=1 KCFLAGS=-Werror modules
python3 "$kernel_build_dir/scripts/clang-tools/gen_compile_commands.py" \
    -d "$task_build_dir" -o "$task_build_dir/compile_commands.json"

for source in "$probe_dir"/*.c; do
    clang-tidy --config-file="$probe_dir/.clang-tidy" -p "$task_build_dir" \
        "$task_build_dir/$(basename -- "$source")"
done
