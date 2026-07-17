# lmu2motec

`lmu2motec` converts Le Mans Ultimate telemetry recordings (`.duckdb`) into
MoTeC Logged Data files (`.ld`) that can be opened in MoTeC i2.

The application provides a simple graphical interface and can export one MoTeC
file per completed lap, all completed laps in one file, or one specific lap.
All supported telemetry channels are included automatically.

## Download

Download the latest executable for your operating system from the
[GitHub Releases](../../releases) page.

No command-line interface or development environment is required.
The corresponding source code for each executable is available from the
source archives attached automatically to the same tagged release.

## How to use

1. Open `lmu2motec`.
2. Select the folder containing your LMU `.duckdb` telemetry files.
3. Select one or more recordings from the list.
4. Choose an output folder.
5. Choose an export mode and, when needed, a lap number.
6. Click **Convert selected**.

Each recording gets its own subfolder in the selected output folder. Depending
on the selected mode, it contains one `.ld` file per lap, a single `.ld` file
with all completed laps, or only the requested lap.

You can stop a conversion at any time. The current `.ld` file will be completed
before the remaining laps are skipped.

## Opening files in MoTeC i2

Open a generated `.ld` file in MoTeC i2 and add the channels you want to inspect
to a Time/Distance Graph, such as:

- `Ground Speed`
- `Engine RPM`
- `Throttle Pos`
- `Brake Pos`

Use time mode when viewing the current per-lap exports.

## Notes

- The final incomplete lap of a recording is not exported.
- Invalidated LMU laps are currently exported like other completed laps.
- All supported telemetry channels are exported automatically.
- The application remembers the last selected source and output folders.

## Licenses

`lmu2motec` is free software released under the
[GNU General Public License version 3 only](LICENSE).

Copyright (C) 2026 Enzo CORSICO.

You may use, study, modify, and redistribute the application under the terms
of that license. Distributed modified versions must remain under GPLv3 and
provide their corresponding source code. The application is provided without
any warranty, to the extent permitted by law.

Third-party license information for Slint and DuckDB is available in
[THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
