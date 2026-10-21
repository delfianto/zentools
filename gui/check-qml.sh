#!/usr/bin/env bash
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
qt_qmake=${QMAKE:-qmake6}
qt_bin_dir=$("$qt_qmake" -query QT_INSTALL_BINS)
for qml_file in "$repo_dir"/gui/qml/*.qml; do
    "$qt_bin_dir/qmlformat" "$qml_file" | diff -u "$qml_file" -
done
"$qt_bin_dir/qmllint" --max-warnings 0 -I "$repo_dir/target/cxxqt/qml_modules" "$repo_dir"/gui/qml/*.qml
