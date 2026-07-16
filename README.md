# lmu2motec

`lmu2motec` converts Le Mans Ultimate telemetry recordings (`.duckdb`) into
MoTeC Logged Data files (`.ld`) that can be opened in MoTeC i2.

The application provides a simple graphical interface and exports one MoTeC
file per completed lap. All supported telemetry channels are included
automatically.

## Download

Download the latest executable for your operating system from the
[GitHub Releases](../../releases) page.

No command-line interface or development environment is required.

## How to use

1. Open `lmu2motec`.
2. Select the folder containing your LMU `.duckdb` telemetry files.
3. Select one or more recordings from the list.
4. Choose an output folder.
5. Click **Convert selected**.

Each recording gets its own subfolder in the selected output folder. One `.ld`
file is generated for every completed lap.

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
