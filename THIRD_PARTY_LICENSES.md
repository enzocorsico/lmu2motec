# Third-party licenses

`lmu2motec` uses the following third-party software.

## Slint

The graphical interface is built with
[Slint](https://slint.dev/). Slint is used and distributed in this application
under the **GNU General Public License version 3 only**.

The complete GPLv3 license text is included in the root
[`LICENSE`](LICENSE) file.

## DuckDB

Telemetry databases are read using [DuckDB](https://duckdb.org/), distributed
under the MIT License.

Copyright 2021-2026 Stichting DuckDB Foundation

The complete license text is included in
[`licenses/DuckDB-MIT.txt`](licenses/DuckDB-MIT.txt).

## LMU car setup exporter and TinyPedal

The LMU `.svm` setup mapping and export logic is adapted from
`lmu_duckdb_carsetup_exporter.py` by S.Victor. That script is based on the
[TinyPedal garage exporter](https://github.com/TinyPedal/TinyPedal/blob/master/tinypedal/process/garage.py).
Both the adapted code and this project are distributed under the GNU General
Public License version 3.
